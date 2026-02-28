// ShaderBG
// Copyright (c) 2025 Harlen Batagelo
// https://github.com/hbatagelo/shaderbg
// SPDX-License-Identifier: GPL-3.0-or-later

//! OpenGL debug output integration (debug builds only).
//!
//! Enables the `GL_KHR_debug`/`GL_ARB_debug_output` extension and
//! installs a debug message callback that forwards driver messages
//! to the application logger.

#[cfg(debug_assertions)]
use {gl::types::*, owo_colors::OwoColorize};

/// Enables OpenGL debug output and installs the debug message callback.
///
/// Must be called after a valid OpenGL context has been made current.
/// Has no effect if the debug extension is unsupported.
#[cfg(debug_assertions)]
pub fn setup_opengl_debugging() {
    if supports_debug_extension() {
        unsafe {
            gl::Enable(gl::DEBUG_OUTPUT);
            gl::Enable(gl::DEBUG_OUTPUT_SYNCHRONOUS);
            gl::DebugMessageCallback(Some(gl_debug_callback), std::ptr::null());
            gl::DebugMessageControl(
                gl::DONT_CARE,
                gl::DONT_CARE,
                gl::DONT_CARE,
                0,
                std::ptr::null(),
                gl::TRUE,
            );
        }
    }
}

/// Returns `true` if the current OpenGL context exposes a debug output extension.
#[cfg(debug_assertions)]
fn supports_debug_extension() -> bool {
    let mut flags = 0;
    unsafe { gl::GetIntegerv(gl::CONTEXT_FLAGS, &mut flags) };

    if flags & gl::CONTEXT_FLAG_DEBUG_BIT as i32 != 0 {
        return true;
    }

    let mut num_extensions = 0;
    unsafe { gl::GetIntegerv(gl::NUM_EXTENSIONS, &mut num_extensions) };

    for i in 0..num_extensions as u32 {
        let ptr = unsafe { gl::GetStringi(gl::EXTENSIONS, i) };
        if !ptr.is_null() {
            let extension = unsafe { std::ffi::CStr::from_ptr(ptr as *const _) }.to_string_lossy();
            if extension == "GL_KHR_debug" || extension == "GL_ARB_debug_output" {
                return true;
            }
        }
    }
    false
}

/// OpenGL debug message callback.
///
/// Maps OpenGL debug severity levels to application log levels.
/// Notification messages are ignored to reduce log noise.
///
/// Note: may be invoked from driver-managed threads.
#[cfg(debug_assertions)]
extern "system" fn gl_debug_callback(
    source: GLenum,
    type_: GLenum,
    id: GLuint,
    severity: GLenum,
    _length: GLsizei,
    message: *const GLchar,
    _user_param: *mut std::ffi::c_void,
) {
    if severity == gl::DEBUG_SEVERITY_NOTIFICATION {
        return;
    }

    let source_str = match source {
        gl::DEBUG_SOURCE_API => "API",
        gl::DEBUG_SOURCE_WINDOW_SYSTEM => "WINDOW_SYSTEM",
        gl::DEBUG_SOURCE_SHADER_COMPILER => "SHADER_COMPILER",
        gl::DEBUG_SOURCE_THIRD_PARTY => "THIRD_PARTY",
        gl::DEBUG_SOURCE_APPLICATION => "APPLICATION",
        gl::DEBUG_SOURCE_OTHER => "OTHER",
        _ => "UNKNOWN",
    };

    let type_str = match type_ {
        gl::DEBUG_TYPE_ERROR => "ERROR",
        gl::DEBUG_TYPE_DEPRECATED_BEHAVIOR => "DEPRECATED_BEHAVIOR",
        gl::DEBUG_TYPE_UNDEFINED_BEHAVIOR => "UNDEFINED_BEHAVIOR",
        gl::DEBUG_TYPE_PORTABILITY => "PORTABILITY",
        gl::DEBUG_TYPE_PERFORMANCE => "PERFORMANCE",
        gl::DEBUG_TYPE_MARKER => "MARKER",
        gl::DEBUG_TYPE_PUSH_GROUP => "PUSH_GROUP",
        gl::DEBUG_TYPE_POP_GROUP => "POP_GROUP",
        gl::DEBUG_TYPE_OTHER => "OTHER",
        _ => "UNKNOWN",
    };

    let severity_str = match severity {
        gl::DEBUG_SEVERITY_HIGH => "HIGH",
        gl::DEBUG_SEVERITY_MEDIUM => "MEDIUM",
        gl::DEBUG_SEVERITY_LOW => "LOW",
        _ => "UNKNOWN",
    };

    let msg = unsafe { std::ffi::CStr::from_ptr(message).to_string_lossy() };

    let binding = "[GL DEBUG]".white();
    let prefix = binding.bold();

    macro_rules! log_gl {
        ($level:ident) => {
            log::$level!(
                "{} source={}, type={}, id={}, severity={}, message={}",
                prefix,
                source_str,
                type_str,
                id,
                severity_str,
                msg
            )
        };
    }

    match severity {
        gl::DEBUG_SEVERITY_HIGH => log_gl!(error),
        gl::DEBUG_SEVERITY_MEDIUM => log_gl!(warn),
        gl::DEBUG_SEVERITY_LOW => log_gl!(info),
        _ => log_gl!(debug),
    }
}
