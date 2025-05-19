use crate::{println, println_at};

use psp::sys::{sceCtrlPeekBufferPositive, sceCtrlSetSamplingCycle, sceCtrlSetSamplingMode, CtrlButtons, CtrlMode, SceCtrlData};

const MID      : i16 = 128;
const DEADZONE : i16 = 16;


pub fn poll_inputs() -> (CtrlButtons, (f32, f32)) {

    let mut pad: SceCtrlData = unsafe { core::mem::zeroed() };

    unsafe {
        sceCtrlSetSamplingCycle(0);
        sceCtrlSetSamplingMode(CtrlMode::Analog);
        sceCtrlPeekBufferPositive(&mut pad, 1); 
    }

    let buttons = pad.buttons;

//     println!("{:?}, {:?}\n{:?}", pad.lx, pad.ly, buttons);

    let lx = pad.lx as i16 - MID;
    let ly = pad.ly as i16 - MID;

    let fx = if lx.abs() > DEADZONE { lx as f32 / 127.0 } else { 0.0 };
    let fy = if ly.abs() > DEADZONE { ly as f32 / 127.0 } else { 0.0 };
    (buttons, (fx, fy))
}
