use psp;

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

