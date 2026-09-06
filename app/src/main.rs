mod api;
mod app;
mod checker;
mod config;
mod history;
mod images;
mod parser;
mod positions;
mod prompt;

fn main() -> eframe::Result {
    eprintln!("[IDV-ASAS] XMODIFIERS={:?}", std::env::var("XMODIFIERS").ok());
    eprintln!("[IDV-ASAS] GTK_IM_MODULE={:?}", std::env::var("GTK_IM_MODULE").ok());

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 720.0])
        .with_title("IDV-ASAS 区域选择辅助系统");
    let mut wgpu_setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    wgpu_setup.instance_descriptor.backends = wgpu::Backends::VULKAN | wgpu::Backends::GL;
    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(wgpu_setup),
            ..Default::default()
        },
        ..Default::default()
    };
    eframe::run_native(
        "IDV-ASAS",
        options,
        Box::new(|cc| Ok(Box::new(app::IdvApp::new(cc)))),
    )
}
