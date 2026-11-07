//! S4 spike: can egui/eframe show debugging data at the sizes the app
//! needs, look like docs/ui.md, and stay idle when nothing happens?
//! Run with `--help` for the options.

mod actions;
mod alloc;
mod app;
mod bench;
mod dumps;
mod flame;
mod mockups;
mod source;
mod theme;
mod tree;

use eframe::egui::{self, ThemePreference};

#[global_allocator]
static A: alloc::Counting = alloc::Counting;

const HELP: &str = "s4-egui [options]
  --view feed|debugger|tree|flame|source   view shown first (default feed)
  --theme light|dark                       default: follow the system
  --backend glow|wgpu                      default: wgpu
  --tree-nodes N      generated tree size (default 100000)
  --expand-all        fetch and expand the whole tree at load
  --expand-sample N   expand the first N orders at load
  --dumps N           feed size (default 10000; 1000 or less starts with the docs/ui.md cards)
  --flame-frames N    flame graph size (default 50000)
  --source-lines N    generated PHP file size (default 5000)
  --load-all          build every view's data at start, not on first show
  --bench SCENARIO    tree-scroll|tree-expand|dumps-scroll|flame-zoom|source-scroll
  --frames N          frames recorded by --bench (default 600)
  --vsync on|off      default: off with --bench, on otherwise
  --screenshot PATH   save a PNG after the first second, then quit
  --hover ACTION      with --screenshot: put the pointer on that action's button
  --size WxH          window size (default 1280x800)
  --quit-after N      print heap and RSS after N frames, then quit
  --a11y-dump         run headless with AccessKit and print the tree summary
  --live-countdown    repaint the session countdown once a second";

fn hover_action(s: &str) -> Option<actions::ActionId> {
    actions::ACTIONS.iter().find(|a| a.name.eq_ignore_ascii_case(&s.replace('-', " "))).map(|a| a.id)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = app::Opts {
        view: app::View::Feed,
        theme: None,
        backend: "wgpu",
        tree_nodes: 100_000,
        dumps: 10_000,
        flame_frames: 50_000,
        source_lines: 5_000,
        expand_all: false,
        expand_sample: 0,
        bench: None,
        screenshot: None,
        hover: None,
        quit_after: None,
        load_all: false,
        live_countdown: false,
    };
    let mut frames = 600;
    let mut vsync: Option<bool> = None;
    let mut size = [1280.0f32, 800.0];
    let mut a11y = false;
    let mut bench_name: Option<String> = None;
    let mut it = args.iter();
    let num = |v: Option<&String>| -> usize { v.and_then(|s| s.replace('_', "").parse().ok()).expect("expected a number") };
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => {
                println!("{HELP}");
                return;
            }
            "--view" => opts.view = app::View::parse(it.next().expect("--view needs a value")).expect("unknown view"),
            "--theme" => {
                opts.theme = Some(match it.next().map(String::as_str) {
                    Some("light") => ThemePreference::Light,
                    Some("dark") => ThemePreference::Dark,
                    _ => panic!("--theme light|dark"),
                })
            }
            "--backend" => {
                opts.backend = match it.next().map(String::as_str) {
                    Some("glow") => "glow",
                    Some("wgpu") => "wgpu",
                    _ => panic!("--backend glow|wgpu"),
                }
            }
            "--tree-nodes" => opts.tree_nodes = num(it.next()),
            "--expand-all" => opts.expand_all = true,
            "--expand-sample" => opts.expand_sample = num(it.next()),
            "--dumps" => opts.dumps = num(it.next()),
            "--flame-frames" => opts.flame_frames = num(it.next()),
            "--source-lines" => opts.source_lines = num(it.next()),
            "--load-all" => opts.load_all = true,
            "--bench" => bench_name = it.next().cloned(),
            "--frames" => frames = num(it.next()),
            "--vsync" => vsync = Some(it.next().map(String::as_str) == Some("on")),
            "--screenshot" => opts.screenshot = it.next().map(Into::into),
            "--hover" => opts.hover = Some(hover_action(it.next().expect("--hover needs an action")).expect("unknown action")),
            "--size" => {
                let v = it.next().expect("--size WxH");
                let (w, h) = v.split_once('x').expect("--size WxH");
                size = [w.parse().unwrap(), h.parse().unwrap()];
            }
            "--quit-after" => opts.quit_after = Some(num(it.next())),
            "--a11y-dump" => a11y = true,
            "--live-countdown" => opts.live_countdown = true,
            other => panic!("unknown option {other}\n{HELP}"),
        }
    }
    if let Some(b) = bench_name {
        let s = bench::Scenario::parse(&b).expect("unknown bench scenario");
        opts.view = match s {
            bench::Scenario::TreeScroll | bench::Scenario::TreeExpand => app::View::Tree,
            bench::Scenario::DumpsScroll => app::View::Feed,
            bench::Scenario::FlameZoom => app::View::Flame,
            bench::Scenario::SourceScroll => app::View::Source,
        };
        opts.bench = Some((s, frames));
    }
    if a11y {
        app::a11y_dump(opts);
        return;
    }
    let vsync = vsync.unwrap_or(opts.bench.is_none());
    let renderer = if opts.backend == "glow" { eframe::Renderer::Glow } else { eframe::Renderer::Wgpu };
    let mut native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(size).with_title("debug-claw S4 spike").with_app_id("debug-claw-s4"),
        renderer,
        system_font_fallback: false,
        persist_window: false,
        ..Default::default()
    };
    native.glow_options.vsync = vsync;
    native.wgpu_options.surface.present_mode = if vsync { eframe::wgpu::PresentMode::AutoVsync } else { eframe::wgpu::PresentMode::AutoNoVsync };
    let backend = opts.backend;
    let theme_pref = opts.theme;
    let t0 = std::time::Instant::now();
    eframe::run_native(
        "debug-claw-s4",
        native,
        Box::new(move |cc| {
            let fonts = theme::install_fonts(&cc.egui_ctx);
            theme::install_styles(&cc.egui_ctx);
            if let Some(t) = theme_pref {
                cc.egui_ctx.set_theme(t);
            }
            let mut info = format!("backend={backend} vsync={vsync} fonts={fonts}");
            if let Some(rs) = &cc.wgpu_render_state {
                let a = rs.adapter.get_info();
                info += &format!(" adapter=\"{}\" driver=\"{}\" driver_info=\"{}\" api={:?} type={:?}", a.name, a.driver, a.driver_info, a.backend, a.device_type);
            }
            if let Some(gl) = &cc.gl {
                use eframe::glow::HasContext;
                // SAFETY: plain string queries on the current GL context.
                let (r, v) = unsafe { (gl.get_parameter_string(eframe::glow::RENDERER), gl.get_parameter_string(eframe::glow::VERSION)) };
                info += &format!(" gl_renderer=\"{r}\" gl_version=\"{v}\"");
            }
            println!("start {info} startup_ms={:.0}", t0.elapsed().as_secs_f64() * 1000.0);
            Ok(Box::new(app::App::new(opts)))
        }),
    )
    .expect("eframe failed");
}
