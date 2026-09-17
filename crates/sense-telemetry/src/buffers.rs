use sense_types::{FrameSample, InputCameraSample, MouseSample, ProcessedMouseSample};

#[derive(Debug, Default)]
pub struct SessionBuffers {
    pub mouse: Vec<MouseSample>,
    pub processed: Vec<ProcessedMouseSample>,
    pub input_camera: Vec<InputCameraSample>,
    pub frames: Vec<FrameSample>,
}

impl SessionBuffers {
    pub fn clear(&mut self) {
        self.mouse.clear();
        self.processed.clear();
        self.input_camera.clear();
        self.frames.clear();
    }
}
