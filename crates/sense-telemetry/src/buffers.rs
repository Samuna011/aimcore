use sense_types::{FrameSample, InputCameraSample, MouseSample};

#[derive(Debug, Default)]
pub struct SessionBuffers {
    pub mouse: Vec<MouseSample>,
    pub input_camera: Vec<InputCameraSample>,
    pub frames: Vec<FrameSample>,
}

impl SessionBuffers {
    pub fn clear(&mut self) {
        self.mouse.clear();
        self.input_camera.clear();
        self.frames.clear();
    }
}
