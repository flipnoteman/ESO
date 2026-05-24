use psp::{
    self,
    sys::{
        SceKernelUtilsMt19937Context, sceKernelUtilsMt19937Init, sceKernelUtilsMt19937UInt,
        sceRtcGetCurrentTick,
    },
};

pub fn rand() -> u32 {
    unsafe {
        let mut ctx = SceKernelUtilsMt19937Context {
            count: 0,
            state: [0; 624],
        };
        let mut tick = 0u64;

        sceRtcGetCurrentTick(&mut tick);
        sceKernelUtilsMt19937Init(&mut ctx, tick as u32);
        sceKernelUtilsMt19937UInt(&mut ctx)
    }
}

/// Calculate the cosine of an angle using the psp VFPU
pub fn vfpu_cosf(x: f32) -> f32 {
    let mut ret_val = 0.0;

    unsafe {
        psp::vfpu_asm!(
            "mtv    {x}, S000",
            "vcst.s S001, VFPU_2_PI",
            "vmul.s S000, S000, S001",
            "vcos.s S000, S000",
            "mfv    {ret}, S000",

            x = inout(reg) x => _,
            ret = out(reg) ret_val,
            options(nostack, nomem),
        );
    }

    ret_val
}

/// Calculate the sin of an angle using the psp VFPU
pub fn vfpu_sinf(x: f32) -> f32 {
    let mut ret_val = 0.0;

    unsafe {
        psp::vfpu_asm!(
            "mtv    {x}, S000",
            "vcst.s S001, VFPU_2_PI",
            "vmul.s S000, S000, S001",
            "vsin.s S000, S000",
            "mfv    {ret}, S000",

            x = inout(reg) x => _,
            ret = out(reg) ret_val,
            options(nostack, nomem),
        );
    }

    ret_val
}

pub fn vfpu_tanf(x: f32) -> f32 {
    let mut ret_val = 0.0;
    unsafe {
        psp::vfpu_asm!(
            "mtv    {x}, S000",
            "vcst.s S001, VFPU_2_PI",
            "vmul.s S000, S000, S001",
            "vsin.s S001, S000",
            "vcos.s S002, S000",
            "vdiv.s S000, S001, S002",
            "mfv    {ret}, S000",
            x = inout(reg) x => _,
            ret = out(reg) ret_val,
            options(nostack, nomem),
        );
    }
    ret_val
}
