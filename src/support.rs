use euclid::default::Size2D;
use glutin::{self, PossiblyCurrent};

use std::ffi::CStr;

pub mod gl {
    pub use self::Gles2 as Gl;
    include!(concat!(env!("OUT_DIR"), "/gl_bindings.rs"));
}

pub struct Gl {
    pub gl: gl::Gl,
    fb: u32,
}

pub fn load(gl_context: &glutin::Context<PossiblyCurrent>) -> Gl {
    let gl =
        gl::Gl::load_with(|ptr| gl_context.get_proc_address(ptr) as *const _);

    let version = unsafe {
        let data = CStr::from_ptr(gl.GetString(gl::VERSION) as *const _)
            .to_bytes()
            .to_vec();
        String::from_utf8(data).unwrap()
    };

    println!("OpenGL version {}", version);

    let mut fb = 0;
    unsafe {
        gl.GenFramebuffers(1, &mut fb);
    }

    Gl { gl, fb }
}

impl Gl {
    #[track_caller]
    pub fn assert_no_error(&self) {
        unsafe {
            assert_eq!(self.gl.GetError(), gl::NO_ERROR);
        }
    }

    pub fn draw_texture(
        &self,
        device: &surfman::Device,
        texture: u32,
        size: Size2D<i32>,
        dest: Size2D<i32>,
    ) {
        unsafe {
            self.gl.BindFramebuffer(gl::READ_FRAMEBUFFER, self.fb);
            self.assert_no_error();
            self.gl.FramebufferTexture2D(
                gl::READ_FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                device.surface_gl_texture_target(),
                //gl::TEXTURE_2D,
                texture,
                0,
            );
            self.assert_no_error();
            self.gl.BlitFramebuffer(
                0, 0, size.width, size.height,
                0, 0, dest.width, dest.height,
                gl::COLOR_BUFFER_BIT,
                gl::LINEAR,
            );
            self.assert_no_error();
            self.gl.BindFramebuffer(gl::READ_FRAMEBUFFER, 0);
            self.assert_no_error();
            self.gl.Flush();
        }
    }
}
