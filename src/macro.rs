
#[macro_export]
macro_rules! log_info {
    ($($expr: tt)*) => {
        println!($($expr)*)
    }
}

#[macro_export]
macro_rules! log_error {
    ($($expr: tt)*) => {
        eprintln!($($expr)*)
    }
}


#[macro_export]
macro_rules! c_str {
    ($s: expr) => {
        concat!($s, "\0").as_ptr() as *const std::ffi::c_char
    }
}

#[macro_export]
macro_rules! check_null {
    ($expr: expr) => {
        match $expr {
            None => None,
            Some(r) if r == std::ptr::null_mut() => None,
            Some(r)  => Some(r),
        }
    };
}

#[macro_export]
macro_rules! jni_method {
    ($jni: ident, $method: tt, $($expr: expr),+) => {
        unsafe {
            check_null!((**$jni.inner()).$method.map(|n| 
                n($jni.inner(), $($expr),+)
            ))
        }
    };
}

#[macro_export]
macro_rules! jni_call_object_method {
    ($jni: ident, $($expr: expr),+) => {
        unsafe {
            check_null!((**$jni.inner()).NewObject.map(|n| 
                n($jni.inner(), $($expr),+)
            ))
        }
    };
}