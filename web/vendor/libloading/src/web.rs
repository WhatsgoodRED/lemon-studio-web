//! Lemon Studio web build: wasm32 cannot load native libraries, so every open fails. Only the API Lemon Studio
//! names is kept.

use std::ffi::OsStr;
use std::marker::PhantomData;

use crate::Error;

#[derive(Debug)]
pub struct Library(());

impl Library {
    pub unsafe fn new<P: AsRef<OsStr>>(_filename: P) -> Result<Library, Error> {
        Err(Error::DlOpenUnknown)
    }
    pub unsafe fn get<T>(&self, _symbol: &[u8]) -> Result<Symbol<'_, T>, Error> {
        Err(Error::DlSymUnknown)
    }
    pub fn close(self) -> Result<(), Error> {
        Ok(())
    }
}

pub struct Symbol<'lib, T: 'lib> {
    pointer: *mut std::ffi::c_void,
    pd: PhantomData<&'lib T>,
}

impl<T> std::ops::Deref for Symbol<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*(&self.pointer as *const *mut _ as *const T) }
    }
}
