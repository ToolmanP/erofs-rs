// Copyright 2024 Yiyang Wu
// SPDX-License-Identifier: MIT or GPL-2.0-or-later

#[repr(i32)]
#[non_exhaustive]
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Copy, Clone, PartialEq)]
/// Errno
pub enum Errno {
    /// None
    NONE = 0,
    /// EPERM
    EPERM,
    ///ENOENT
    ENOENT,
    ///ESRCH
    ESRCH,
    ///EINTR
    EINTR,
    ///EIO
    EIO,
    ///ENXIO
    ENXIO,
    ///E2BIG
    E2BIG,
    ///ENOEXEC
    ENOEXEC,
    ///EBADF
    EBADF,
    ///ECHILD
    ECHILD,
    ///EAGAIN
    EAGAIN,
    ///ENOMEM
    ENOMEM,
    ///EACCES
    EACCES,
    ///EFAULT
    EFAULT,
    ///ENOTBLK
    ENOTBLK,
    ///EBUSY
    EBUSY,
    ///EEXIST
    EEXIST,
    ///EXDEV
    EXDEV,
    ///ENODEV
    ENODEV,
    ///ENOTDIR
    ENOTDIR,
    ///EISDIR
    EISDIR,
    ///EINVAL
    EINVAL,
    ///ENFILE
    ENFILE,
    ///EMFILE
    EMFILE,
    ///ENOTTY
    ENOTTY,
    ///ETXTBSY
    ETXTBSY,
    ///EFBIG
    EFBIG,
    ///ENOSPC
    ENOSPC,
    ///ESPIPE
    ESPIPE,
    ///EROFS
    EROFS,
    ///EMLINK
    EMLINK,
    ///EPIPE
    EPIPE,
    ///EDOM
    EDOM,
    ///ERANGE
    ERANGE,
    ///EDEADLK
    EDEADLK,
    ///ENAMETOOLONG
    ENAMETOOLONG,
    ///ENOLCK
    ENOLCK,
    ///ENOSYS
    ENOSYS,
    ///ENOTEMPTY
    ENOTEMPTY,
    ///ELOOP
    ELOOP,
    ///ENOMSG = 42
    ENOMSG = 42,
    ///EIDRM
    EIDRM,
    ///ECHRNG
    ECHRNG,
    ///EL2NSYNC
    EL2NSYNC,
    ///EL3HLT
    EL3HLT,
    ///EL3RST
    EL3RST,
    ///ELNRNG
    ELNRNG,
    ///EUNATCH
    EUNATCH,
    ///ENOCSI
    ENOCSI,
    ///EL2HLT
    EL2HLT,
    ///EBADE
    EBADE,
    ///EBADR
    EBADR,
    ///EXFULL
    EXFULL,
    ///ENOANO
    ENOANO,
    ///EBADRQC
    EBADRQC,
    ///EBADSLT
    EBADSLT,
    ///EBFONT = 59
    EBFONT = 59,
    ///ENOSTR
    ENOSTR,
    ///ENODATA
    ENODATA,
    ///ETIME
    ETIME,
    ///ENOSR
    ENOSR,
    ///ENONET
    ENONET,
    ///ENOPKG
    ENOPKG,
    ///EREMOTE
    EREMOTE,
    ///ENOLINK
    ENOLINK,
    ///EADV
    EADV,
    ///ESRMNT
    ESRMNT,
    ///ECOMM
    ECOMM,
    ///EPROTO
    EPROTO,
    ///EMULTIHOP
    EMULTIHOP,
    ///EDOTDOT
    EDOTDOT,
    ///EBADMSG
    EBADMSG,
    ///EOVERFLOW
    EOVERFLOW,
    ///ENOTUNIQ
    ENOTUNIQ,
    ///EBADFD
    EBADFD,
    ///EREMCHG
    EREMCHG,
    ///ELIBACC
    ELIBACC,
    ///ELIBBAD
    ELIBBAD,
    ///ELIBSCN
    ELIBSCN,
    ///ELIBMAX
    ELIBMAX,
    ///ELIBEXEC
    ELIBEXEC,
    ///EILSEQ
    EILSEQ,
    ///ERESTART
    ERESTART,
    ///ESTRPIPE
    ESTRPIPE,
    ///EUSERS
    EUSERS,
    ///ENOTSOCK
    ENOTSOCK,
    ///EDESTADDRREQ
    EDESTADDRREQ,
    ///EMSGSIZE
    EMSGSIZE,
    ///EPROTOTYPE
    EPROTOTYPE,
    ///ENOPROTOOPT
    ENOPROTOOPT,
    ///EPROTONOSUPPORT
    EPROTONOSUPPORT,
    ///ESOCKTNOSUPPORT
    ESOCKTNOSUPPORT,
    ///EOPNOTSUPP
    EOPNOTSUPP,
    ///EPFNOSUPPORT
    EPFNOSUPPORT,
    ///EAFNOSUPPORT
    EAFNOSUPPORT,
    ///EADDRINUSE
    EADDRINUSE,
    ///EADDRNOTAVAIL
    EADDRNOTAVAIL,
    ///ENETDOWN
    ENETDOWN,
    ///ENETUNREACH
    ENETUNREACH,
    ///ENETRESET
    ENETRESET,
    ///ECONNABORTED
    ECONNABORTED,
    ///ECONNRESET
    ECONNRESET,
    ///ENOBUFS
    ENOBUFS,
    ///EISCONN
    EISCONN,
    ///ENOTCONN
    ENOTCONN,
    ///ESHUTDOWN
    ESHUTDOWN,
    ///ETOOMANYREFS
    ETOOMANYREFS,
    ///ETIMEDOUT
    ETIMEDOUT,
    ///ECONNREFUSED
    ECONNREFUSED,
    ///EHOSTDOWN
    EHOSTDOWN,
    ///EHOSTUNREACH
    EHOSTUNREACH,
    ///EALREADY
    EALREADY,
    ///EINPROGRESS
    EINPROGRESS,
    ///ESTALE
    ESTALE,
    ///EUCLEAN
    EUCLEAN,
    ///ENOTNAM
    ENOTNAM,
    ///ENAVAIL
    ENAVAIL,
    ///EISNAM
    EISNAM,
    ///EREMOTEIO
    EREMOTEIO,
    ///EDQUOT
    EDQUOT,
    ///ENOMEDIUM
    ENOMEDIUM,
    ///EMEDIUMTYPE
    EMEDIUMTYPE,
    ///ECANCELED
    ECANCELED,
    ///ENOKEY
    ENOKEY,
    ///EKEYEXPIRED
    EKEYEXPIRED,
    ///EKEYREVOKED
    EKEYREVOKED,
    ///EKEYREJECTED
    EKEYREJECTED,
    ///EOWNERDEAD
    EOWNERDEAD,
    ///ENOTRECOVERABLE
    ENOTRECOVERABLE,
    ///ERFKILL
    ERFKILL,
    ///EHWPOISON
    EHWPOISON,
    ///EUNKNOWN
    EUNKNOWN,
}

impl From<i32> for Errno {
    fn from(value: i32) -> Self {
        if (-value) <= 0 || (-value) > Errno::EUNKNOWN as i32 {
            Errno::EUNKNOWN
        } else {
            // Safety: The value is guaranteed to be a valid errno and the memory
            // layout is the same for both types.
            unsafe { core::mem::transmute::<i32, Errno>(value) }
        }
    }
}

pub(crate) use Errno::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]

    fn test_proper_errno_value() {
        assert_eq!(Errno::ERANGE as i32, 34);
        assert_eq!(Errno::ENODATA as i32, 61);
    }
}
