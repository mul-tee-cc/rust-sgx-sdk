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

//! This module contains constructs to work with 16-bit characters (UCS-2 or UTF-16)
#![allow(dead_code)]

use crate::marker::PhantomData;
use crate::num::NonZeroU16;
use crate::ptr::NonNull;

/// A safe iterator over a LPWSTR
/// (aka a pointer to a series of UTF-16 code units terminated by a NULL).
pub struct WStrUnits<'a> {
    // The pointer must never be null...
    lpwstr: NonNull<u16>,
    // ...and the memory it points to must be valid for this lifetime.
    lifetime: PhantomData<&'a [u16]>,
}

impl WStrUnits<'_> {
    /// Create the iterator. Returns `None` if `lpwstr` is null.
    ///
    /// SAFETY: `lpwstr` must point to a null-terminated wide string that lives
    /// at least as long as the lifetime of this struct.
    pub unsafe fn new(lpwstr: *const u16) -> Option<Self> {
        Some(Self { lpwstr: NonNull::new(lpwstr as _)?, lifetime: PhantomData })
    }

    pub fn peek(&self) -> Option<NonZeroU16> {
        // SAFETY: It's always safe to read the current item because we don't
        // ever move out of the array's bounds.
        unsafe { NonZeroU16::new(*self.lpwstr.as_ptr()) }
    }

    /// Advance the iterator while `predicate` returns true.
    /// Returns the number of items it advanced by.
    pub fn advance_while<P: FnMut(NonZeroU16) -> bool>(&mut self, mut predicate: P) -> usize {
        let mut counter = 0;
        while let Some(w) = self.peek() {
            if !predicate(w) {
                break;
            }
            counter += 1;
            self.next();
        }
        counter
    }
}

impl Iterator for WStrUnits<'_> {
    // This can never return zero as that marks the end of the string.
    type Item = NonZeroU16;
    fn next(&mut self) -> Option<NonZeroU16> {
        // SAFETY: If NULL is reached we immediately return.
        // Therefore it's safe to advance the pointer after that.
        unsafe {
            let next = self.peek()?;
            self.lpwstr = NonNull::new_unchecked(self.lpwstr.as_ptr().add(1));
            Some(next)
        }
    }
}
