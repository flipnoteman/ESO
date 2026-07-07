use psp::{
    self,
    sys::{
        SceKernelUtilsMt19937Context, ScePspFVector3, sceKernelUtilsMt19937Init,
        sceKernelUtilsMt19937UInt, sceRtcGetCurrentTick,
    },
};

pub fn vlength(v: ScePspFVector3) -> f32 {
    let mut ret_val = 0.0;

    // Computing dot product with self gives x*x + y*y ... which is what we need
    let dot = vdot(v, v);

    // Compute sqrt of summed squares
    let ret_val = sqrt(dot);

    ret_val
}

pub fn sqrt(x: f32) -> f32 {
    let mut ret_val = 0.0;

    unsafe {
        psp::vfpu_asm!(
            "mtv {x}, S000",
            "vsqrt.s S001, S000",
            "mfv {ret}, S001",

            x = inout(reg) x => _,
            ret = out(reg) ret_val,
            options(nostack, nomem),
        );
    }

    ret_val
}

pub fn vdot(v: ScePspFVector3, w: ScePspFVector3) -> f32 {
    // NOTE: lv.s/sv.s (VFPU<->memory) are unreliable on this target, so all vector
    // ops move data through GPRs via mtv/mfv instead.
    let (vx, vy, vz) = (v.x, v.y, v.z);
    let (wx, wy, wz) = (w.x, w.y, w.z);
    let mut ret_val = 0.0f32;

    unsafe {
        psp::vfpu_asm!(
            "mtv {vx}, S000",
            "mtv {vy}, S001",
            "mtv {vz}, S002",
            "mtv {wx}, S010",
            "mtv {wy}, S011",
            "mtv {wz}, S012",

            "vdot.t S020, C000, C010",

            "mfv {ret}, S020",

            vx = in(reg) vx, vy = in(reg) vy, vz = in(reg) vz,
            wx = in(reg) wx, wy = in(reg) wy, wz = in(reg) wz,
            ret = out(reg) ret_val,
            options(nostack, nomem),
        );
    }

    ret_val
}

/// Performs v-w on psp vfpu (mtv/mfv; lv.s/sv.s unreliable on this target)
pub fn vsub(v: ScePspFVector3, w: ScePspFVector3) -> ScePspFVector3 {
    let (vx, vy, vz) = (v.x, v.y, v.z);
    let (wx, wy, wz) = (w.x, w.y, w.z);
    let mut rx = 0.0f32;
    let mut ry = 0.0f32;
    let mut rz = 0.0f32;

    unsafe {
        psp::vfpu_asm!(
            "mtv {vx}, S000",
            "mtv {vy}, S001",
            "mtv {vz}, S002",
            "mtv {wx}, S010",
            "mtv {wy}, S011",
            "mtv {wz}, S012",

            "vsub.t C020, C000, C010",

            "mfv {rx}, S020",
            "mfv {ry}, S021",
            "mfv {rz}, S022",

            vx = in(reg) vx, vy = in(reg) vy, vz = in(reg) vz,
            wx = in(reg) wx, wy = in(reg) wy, wz = in(reg) wz,
            rx = out(reg) rx, ry = out(reg) ry, rz = out(reg) rz,
            options(nostack, nomem),
        );
    }

    ScePspFVector3 { x: rx, y: ry, z: rz }
}

/// Performs v+w on psp vfpu (mtv/mfv; lv.s/sv.s unreliable on this target)
pub fn vadd(v: ScePspFVector3, w: ScePspFVector3) -> ScePspFVector3 {
    let (vx, vy, vz) = (v.x, v.y, v.z);
    let (wx, wy, wz) = (w.x, w.y, w.z);
    let mut rx = 0.0f32;
    let mut ry = 0.0f32;
    let mut rz = 0.0f32;

    unsafe {
        psp::vfpu_asm!(
            // v vector into column C000 (S000,S001,S002)
            "mtv {vx}, S000",
            "mtv {vy}, S001",
            "mtv {vz}, S002",
            // w vector into column C010 (S010,S011,S012)
            "mtv {wx}, S010",
            "mtv {wy}, S011",
            "mtv {wz}, S012",

            "vadd.t C020, C000, C010",

            "mfv {rx}, S020",
            "mfv {ry}, S021",
            "mfv {rz}, S022",

            vx = in(reg) vx, vy = in(reg) vy, vz = in(reg) vz,
            wx = in(reg) wx, wy = in(reg) wy, wz = in(reg) wz,
            rx = out(reg) rx, ry = out(reg) ry, rz = out(reg) rz,
            options(nostack, nomem),
        );
    }

    ScePspFVector3 { x: rx, y: ry, z: rz }
}

/// Negates given vector using psp vfpu (mtv/mfv; lv.s/sv.s unreliable on this target)
pub fn vneg(v: ScePspFVector3) -> ScePspFVector3 {
    let (vx, vy, vz) = (v.x, v.y, v.z);
    let mut rx = 0.0f32;
    let mut ry = 0.0f32;
    let mut rz = 0.0f32;

    unsafe {
        psp::vfpu_asm!(
            "mtv {vx}, S000",
            "mtv {vy}, S001",
            "mtv {vz}, S002",

            "vneg.t C010, C000",

            "mfv {rx}, S010",
            "mfv {ry}, S011",
            "mfv {rz}, S012",

            vx = in(reg) vx, vy = in(reg) vy, vz = in(reg) vz,
            rx = out(reg) rx, ry = out(reg) ry, rz = out(reg) rz,
            options(nostack, nomem),
        );
    }

    ScePspFVector3 { x: rx, y: ry, z: rz }
}

/// Returns normalized vector with psp vfpu (mtv/mfv; lv.s/sv.s unreliable on this target)
pub fn vnormalize(v: ScePspFVector3) -> ScePspFVector3 {
    let (vx, vy, vz) = (v.x, v.y, v.z);
    let mut rx = 0.0f32;
    let mut ry = 0.0f32;
    let mut rz = 0.0f32;

    unsafe {
        psp::vfpu_asm!(
            "mtv {vx}, S000",
            "mtv {vy}, S001",
            "mtv {vz}, S002",

            // Square and add components
            "vdot.t S010, C000, C000",
            // Compute 1 / sqrt(r)
            "vrsq.s S010, S010",
            // Scalar multiply each value by 1 / sqrt(r)
            "vscl.t C020, C000, S010",

            "mfv {rx}, S020",
            "mfv {ry}, S021",
            "mfv {rz}, S022",

            vx = in(reg) vx, vy = in(reg) vy, vz = in(reg) vz,
            rx = out(reg) rx, ry = out(reg) ry, rz = out(reg) rz,
            options(nostack, nomem),
        );
    }

    ScePspFVector3 { x: rx, y: ry, z: rz }
}

/// Performs full cross product using psp vfpu (mtv/mfv; lv.s/sv.s unreliable on this target)
pub fn vcross(v: ScePspFVector3, w: ScePspFVector3) -> ScePspFVector3 {
    let (vx, vy, vz) = (v.x, v.y, v.z);
    let (wx, wy, wz) = (w.x, w.y, w.z);
    let mut rx = 0.0f32;
    let mut ry = 0.0f32;
    let mut rz = 0.0f32;

    unsafe {
        psp::vfpu_asm!(
            "mtv {vx}, S000",
            "mtv {vy}, S001",
            "mtv {vz}, S002",
            "mtv {wx}, S010",
            "mtv {wy}, S011",
            "mtv {wz}, S012",

            // full cross product
            "vcrsp.t C020, C000, C010",

            "mfv {rx}, S020",
            "mfv {ry}, S021",
            "mfv {rz}, S022",

            vx = in(reg) vx, vy = in(reg) vy, vz = in(reg) vz,
            wx = in(reg) wx, wy = in(reg) wy, wz = in(reg) wz,
            rx = out(reg) rx, ry = out(reg) ry, rz = out(reg) rz,
            options(nostack, nomem),
        );
    }

    ScePspFVector3 { x: rx, y: ry, z: rz }
}

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

/// Calculate the arctangent of x (radians) using the psp VFPU.
///
/// The VFPU has no direct atan instruction, so this uses the identity
/// atan(x) = asin(x / sqrt(1 + x²)). `vasin.s` returns the arcsine scaled to
/// units of π/2, so the result is multiplied back up by VFPU_PI_2 to get radians.
pub fn vfpu_atanf(x: f32) -> f32 {
    let mut ret_val = 0.0;
    unsafe {
        psp::vfpu_asm!(
            "mtv    {x}, S000",
            // 1 + x²
            "vmul.s S001, S000, S000",
            "vone.s S002",
            "vadd.s S001, S001, S002",
            // x / sqrt(1 + x²)  ∈ (-1, 1)
            "vrsq.s S001, S001",
            "vmul.s S000, S000, S001",
            // asin, scaled from [-1,1] (units of π/2) back to radians
            "vasin.s S000, S000",
            "vcst.s S001, VFPU_PI_2",
            "vmul.s S000, S000, S001",
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

/// Scale vector v by scalar s using VFPU vscl.t (mtv/mfv; lv.s/sv.s unreliable on this target)
pub fn vscale(v: ScePspFVector3, s: f32) -> ScePspFVector3 {
    let (vx, vy, vz) = (v.x, v.y, v.z);
    let mut rx = 0.0f32;
    let mut ry = 0.0f32;
    let mut rz = 0.0f32;

    unsafe {
        psp::vfpu_asm!(
            "mtv {vx}, S000",
            "mtv {vy}, S001",
            "mtv {vz}, S002",
            "mtv {s},  S010",

            "vscl.t C020, C000, S010",

            "mfv {rx}, S020",
            "mfv {ry}, S021",
            "mfv {rz}, S022",

            vx = in(reg) vx, vy = in(reg) vy, vz = in(reg) vz,
            s  = in(reg) s,
            rx = out(reg) rx, ry = out(reg) ry, rz = out(reg) rz,
            options(nostack, nomem),
        );
    }

    ScePspFVector3 { x: rx, y: ry, z: rz }
}
