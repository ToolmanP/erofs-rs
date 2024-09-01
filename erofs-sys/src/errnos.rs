#[repr(i32)]
#[non_exhaustive]
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum Errno {
    NONE = 0,
    EPERM,
    ENOENT,
    ESRCH,
    EINTR,
    EIO,
    ENXIO,
    E2BIG,
    ENOEXEC,
    EBADF,
    ECHILD,
    EAGAIN,
    ENOMEM,
    EACCES,
    EFAULT,
    ENOTBLK,
    EBUSY,
    EEXIST,
    EXDEV,
    ENODEV,
    ENOTDIR,
    EISDIR,
    EINVAL,
    ENFILE,
    EMFILE,
    ENOTTY,
    ETXTBSY,
    EFBIG,
    ENOSPC,
    ESPIPE,
    EROFS,
    EMLINK,
    EPIPE,
    EDOM,
    ERANGE,
    EDEADLK,
    ENAMETOOLONG,
    ENOLCK,
    ENOSYS,
    ENOTEMPTY,
    ELOOP,
    ENOMSG = 42,
    EIDRM,
    ECHRNG,
    EL2NSYNC,
    EL3HLT,
    EL3RST,
    ELNRNG,
    EUNATCH,
    ENOCSI,
    EL2HLT,
    EBADE,
    EBADR,
    EXFULL,
    ENOANO,
    EBADRQC,
    EBADSLT,
    EBFONT = 59,
    ENOSTR,
    ENODATA,
    ETIME,
    ENOSR,
    ENONET,
    ENOPKG,
    EREMOTE,
    ENOLINK,
    EADV,
    ESRMNT,
    ECOMM,
    EPROTO,
    EMULTIHOP,
    EDOTDOT,
    EBADMSG,
    EOVERFLOW,
    ENOTUNIQ,
    EBADFD,
    EREMCHG,
    ELIBACC,
    ELIBBAD,
    ELIBSCN,
    ELIBMAX,
    ELIBEXEC,
    EILSEQ,
    ERESTART,
    ESTRPIPE,
    EUSERS,
    ENOTSOCK,
    EDESTADDRREQ,
    EMSGSIZE,
    EPROTOTYPE,
    ENOPROTOOPT,
    EPROTONOSUPPORT,
    ESOCKTNOSUPPORT,
    EOPNOTSUPP,
    EPFNOSUPPORT,
    EAFNOSUPPORT,
    EADDRINUSE,
    EADDRNOTAVAIL,
    ENETDOWN,
    ENETUNREACH,
    ENETRESET,
    ECONNABORTED,
    ECONNRESET,
    ENOBUFS,
    EISCONN,
    ENOTCONN,
    ESHUTDOWN,
    ETOOMANYREFS,
    ETIMEDOUT,
    ECONNREFUSED,
    EHOSTDOWN,
    EHOSTUNREACH,
    EALREADY,
    EINPROGRESS,
    ESTALE,
    EUCLEAN,
    ENOTNAM,
    ENAVAIL,
    EISNAM,
    EREMOTEIO,
    EDQUOT,
    ENOMEDIUM,
    EMEDIUMTYPE,
    ECANCELED,
    ENOKEY,
    EKEYEXPIRED,
    EKEYREVOKED,
    EKEYREJECTED,
    EOWNERDEAD,
    ENOTRECOVERABLE,
    ERFKILL,
    EHWPOISON,
    EUNKNOWN,
}

impl From<i32> for Errno {
    fn from(value: i32) -> Self {
        if (-value) < 0 || (-value) > Errno::EUNKNOWN as i32 {
            Errno::EUNKNOWN
        } else {
            // Safety: The value is guaranteed to be a valid errno and the memory
            // layout is the same for both types.
            unsafe { core::mem::transmute(value) }
        }
    }
}

impl From<Errno> for i32 {
    fn from(value: Errno) -> Self {
        -(value as i32)
    }
}

/// Replacement for ERR_PTR in Linux Kernel.
impl From<Errno> for *const core::ffi::c_void {
    fn from(value: Errno) -> Self {
        (-(value as core::ffi::c_long)) as *const core::ffi::c_void
    }
}

/// Replacement for PTR_ERR in Linux Kernel.
impl From<*const core::ffi::c_void> for Errno {
    fn from(value: *const core::ffi::c_void) -> Self {
        (value as i32).into()
    }
}

/// Replacement for IS_ERR in Linux Kernel.
#[inline(always)]
fn is_value_err(value: *const core::ffi::c_void) -> bool {
    (value as core::ffi::c_ulong) >= (-4095 as core::ffi::c_long) as core::ffi::c_ulong
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]

    fn test_proper_errno_value() {
        assert_eq!(Errno::ERANGE as i32, 34);
        assert_eq!(Errno::ENODATA as i32, 61);
    }
}
