// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License..

use crate::session::Responder;
use crate::QveReportInfo;
use core::mem::{self, ManuallyDrop};
use core::slice;
use sgx_dcap_ra_msg::{DcapMRaMsg2, DcapRaMsg3};
use sgx_trts::trts::{is_within_enclave, is_within_host};
use sgx_types::error::SgxStatus;
use sgx_types::types::time_t;
use sgx_types::types::{
    CDcapMRaMsg2, CDcapRaMsg1, CDcapRaMsg3, CEnclaveIdentity, Ec256PublicKey, Key128bit,
    QlQvResult, Quote3, QuoteNonce, RaContext, RaKeyType, Report, TargetInfo,
};

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sgx_mra_responder_init(context: *mut RaContext) -> SgxStatus {
    if context.is_null() {
        return SgxStatus::InvalidParameter;
    }

    let responder = match Responder::new() {
        Ok(responder) => responder,
        Err(e) => return e,
    };

    *context = responder.into_raw();
    SgxStatus::Success
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sgx_dcap_mra_proc_msg1(
    context: RaContext,
    msg1: *const CDcapRaMsg1,
    qe_target: *const TargetInfo,
    pub_key_b: *mut Ec256PublicKey,
    report: *mut Report,
    nonce: *mut QuoteNonce,
) -> SgxStatus {
    if msg1.is_null()
        || qe_target.is_null()
        || pub_key_b.is_null()
        || report.is_null()
        || nonce.is_null()
    {
        return SgxStatus::InvalidParameter;
    }

    if !is_within_enclave(msg1 as *const u8, mem::size_of::<CDcapRaMsg1>())
        || !is_within_enclave(qe_target as *const u8, mem::size_of::<TargetInfo>())
        || !is_within_enclave(pub_key_b as *const u8, mem::size_of::<Ec256PublicKey>())
        || !is_within_enclave(report as *const u8, mem::size_of::<Report>())
        || !is_within_enclave(nonce as *const u8, mem::size_of::<QuoteNonce>())
    {
        return SgxStatus::InvalidParameter;
    }

    let qe_target = &*qe_target;
    let msg1 = (&*msg1).into();

    let responder = ManuallyDrop::new(Responder::from_raw(context));
    let (pub_key, rpt, rand) = match responder.process_msg1(&msg1, qe_target) {
        Ok(r) => r,
        Err(e) => return e,
    };

    *pub_key_b = pub_key.into();
    *report = rpt;
    *nonce = rand;
    SgxStatus::Success
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sgx_dcap_mra_get_msg2(
    context: RaContext,
    qe_report: *const Report,
    msg2: *mut CDcapMRaMsg2,
    msg2_size: u32,
) -> SgxStatus {
    if qe_report.is_null() || msg2.is_null() {
        return SgxStatus::InvalidParameter;
    }

    if usize::MAX - (msg2 as usize) < msg2_size as usize
        || msg2_size < (mem::size_of::<CDcapMRaMsg2>() + mem::size_of::<Quote3>()) as u32
    {
        return SgxStatus::InvalidParameter;
    }

    if !(is_within_enclave(msg2 as *const u8, msg2_size as usize)
        || is_within_host(msg2 as *const u8, msg2_size as usize))
    {
        return SgxStatus::InvalidParameter;
    }

    if !is_within_enclave(qe_report as *const u8, mem::size_of::<Report>()) {
        return SgxStatus::InvalidParameter;
    }

    let qe_report = &*qe_report;
    let c_msg2 = &mut *msg2;
    let quote_size = c_msg2.quote_size;

    if !DcapMRaMsg2::check_quote_len(quote_size as usize) {
        return SgxStatus::InvalidParameter;
    }
    if msg2_size != mem::size_of::<CDcapMRaMsg2>() as u32 + quote_size {
        return SgxStatus::InvalidParameter;
    }

    let quote = slice::from_raw_parts(&c_msg2.quote as *const _ as *const u8, quote_size as usize);
    let responder = ManuallyDrop::new(Responder::from_raw(context));
    let msg2 = match responder.generate_msg2(qe_report, quote) {
        Ok(msg) => msg,
        Err(e) => return e,
    };

    c_msg2.mac = msg2.mac;
    c_msg2.g_b = msg2.pub_key_b.into();
    c_msg2.kdf_id = msg2.kdf_id;
    SgxStatus::Success
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sgx_dcap_mra_proc_msg3(
    context: RaContext,
    msg3: *const CDcapRaMsg3,
    msg3_size: u32,
    expiration_time: time_t,
    collateral_expiration_status: u32,
    quote_verification_result: QlQvResult,
    qve_nonce: *const QuoteNonce,
    qve_report: *const Report,
    supplemental_data: *const u8,
    supplemental_data_size: u32,
) -> SgxStatus {
    if msg3.is_null() || qve_nonce.is_null() || qve_report.is_null() {
        return SgxStatus::InvalidParameter;
    }

    if supplemental_data.is_null() && supplemental_data_size != 0 {
        return SgxStatus::InvalidParameter;
    }
    if !supplemental_data.is_null() && supplemental_data_size == 0 {
        return SgxStatus::InvalidParameter;
    }

    if usize::MAX - (msg3 as usize) < msg3_size as usize
        || msg3_size < (mem::size_of::<CDcapRaMsg3>() + mem::size_of::<Quote3>()) as u32
    {
        return SgxStatus::InvalidParameter;
    }

    if !(is_within_enclave(msg3 as *const u8, msg3_size as usize)
        || is_within_host(msg3 as *const u8, msg3_size as usize))
    {
        return SgxStatus::InvalidParameter;
    }

    if !is_within_enclave(qve_nonce as *const u8, mem::size_of::<QuoteNonce>())
        || !is_within_enclave(qve_report as *const u8, mem::size_of::<Report>())
    {
        return SgxStatus::InvalidParameter;
    }

    if !supplemental_data.is_null()
        && !is_within_enclave(supplemental_data, supplemental_data_size as usize)
    {
        return SgxStatus::InvalidParameter;
    }

    let qve_nonce = *qve_nonce;
    let qve_report = &*qve_report;

    let msg3_slice = slice::from_raw_parts(msg3 as *const u8, msg3_size as usize);
    let msg3 = match DcapRaMsg3::from_slice(msg3_slice) {
        Ok(msg) => msg,
        Err(e) => return e,
    };

    let supplemental_data = if !supplemental_data.is_null() {
        Some(slice::from_raw_parts(
            supplemental_data,
            supplemental_data_size as usize,
        ))
    } else {
        None
    };

    let qve_report_info = QveReportInfo {
        qve_report,
        expiration_time,
        collateral_expiration_status,
        quote_verification_result,
        qve_nonce,
        supplemental_data,
    };

    let responder = ManuallyDrop::new(Responder::from_raw(context));
    let _ = match responder.process_msg3(&msg3, &qve_report_info) {
        Ok(identity) => identity,
        Err(e) => return e,
    };

    SgxStatus::Success
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sgx_mra_responder_get_peer_identity(
    context: RaContext,
    quote_verification_result: *mut QlQvResult,
    enclave_identity: *mut CEnclaveIdentity,
) -> SgxStatus {
    if quote_verification_result.is_null() || enclave_identity.is_null() {
        return SgxStatus::InvalidParameter;
    }

    if !is_within_enclave(
        quote_verification_result as *const u8,
        mem::size_of::<QlQvResult>(),
    ) {
        return SgxStatus::InvalidParameter;
    }
    if !is_within_enclave(
        enclave_identity as *const u8,
        mem::size_of::<CEnclaveIdentity>(),
    ) {
        return SgxStatus::InvalidParameter;
    }

    let responder = ManuallyDrop::new(Responder::from_raw(context));
    let (qv_result, identity) = match responder.get_peer_identity() {
        Ok(identity) => identity,
        Err(e) => return e,
    };

    *quote_verification_result = qv_result;
    *enclave_identity = identity.into();
    SgxStatus::Success
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sgx_mra_responder_get_keys(
    context: RaContext,
    key_type: RaKeyType,
    key: *mut Key128bit,
) -> SgxStatus {
    if key.is_null() {
        return SgxStatus::InvalidParameter;
    }

    if !is_within_enclave(key as *const u8, mem::size_of::<Key128bit>()) {
        return SgxStatus::InvalidParameter;
    }

    let responder = ManuallyDrop::new(Responder::from_raw(context));
    let ra_key = match responder.get_keys(key_type) {
        Ok(key) => key.key,
        Err(e) => return e,
    };

    *key = ra_key;
    SgxStatus::Success
}

/// # Safety
#[no_mangle]
pub unsafe extern "C" fn sgx_mra_responder_close(context: RaContext) -> SgxStatus {
    let responder = Responder::from_raw(context);
    drop(responder);
    SgxStatus::Success
}
