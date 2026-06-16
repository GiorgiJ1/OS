#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use aios_assistant::Assistant;
use aios_memory::Database;
use eframe::egui;
use tokio::sync::mpsc;
use tracing::info;

struct AiosApp {
    input:            String,
    messages:         Vec<ChatMsg>,
    chat_open:        bool,
    duck_frame:       usize,
    duck_x:           f32,
    duck_y:           f32,
    duck_vx:          f32,
    frame_timer:      f32,
    anim_state:       AnimState,
    db_path:          String,
    conv_id:          uuid::Uuid,
    response_rx:      mpsc::Receiver<BackendMsg>,
    response_tx:      mpsc::Sender<BackendMsg>,
    is_thinking:      bool,
    current_response: String,
    status_log:       Vec<String>,
}

#[derive(Clone)]
struct ChatMsg {
    role:    Role,
    content: String,
}

#[derive(Clone, PartialEq)]
enum Role { User, Assistant }

#[derive(Clone, PartialEq)]
enum AnimState { WalkRight, WalkLeft, Thinking, Idle }

enum BackendMsg {
    Token(String),
    Done,
    Error(String),
    Status(String),
}

const PALETTE: [Option<egui::Color32>; 7] = [
    None,
    Some(egui::Color32::from_rgb(245, 197, 24)),
    Some(egui::Color32::from_rgb(224, 123, 16)),
    Some(egui::Color32::from_rgb(255, 248, 220)),
    Some(egui::Color32::from_rgb(26,  26,  26)),
    Some(egui::Color32::from_rgb(200, 160, 16)),
    Some(egui::Color32::from_rgb(240, 165,  0)),
];

const DUCK_R1: &[&str] = &[
    "0000001111000000","0000011111100000","0000111111110000","0000111411110000",
    "0000111111112220","0001111111111220","0011133111111000","0011133111110000",
    "0001111111100000","0000111111000000","0000111111000000","0000011110000000",
    "0000022200000000","0000202000000000","0000000000000000","0000000000000000",
];
const DUCK_R2: &[&str] = &[
    "0000001111000000","0000011111100000","0000111111110000","0000111411110000",
    "0000111111112220","0001111111111220","0011133111111000","0011133111110000",
    "0001111111100000","0000111111000000","0000111111000000","0000011110000000",
    "0000202000000000","0000022200000000","0000000000000000","0000000000000000",
];
const DUCK_L1: &[&str] = &[
    "0000001111000000","0000011111100000","0000111111110000","0000111141110000",
    "0222211111110000","0222111111111000","0000111111331100","0000011111331100",
    "0000001111111000","0000000111111000","0000000111111000","0000000111100000",
    "0000000022200000","0000000020200000","0000000000000000","0000000000000000",
];
const DUCK_L2: &[&str] = &[
    "0000001111000000","0000011111100000","0000111111110000","0000111141110000",
    "0222211111110000","0222111111111000","0000111111331100","0000011111331100",
    "0000001111111000","0000000111111000","0000000111111000","0000000111100000",
    "0000000020200000","0000000022200000","0000000000000000","0000000000000000",
];
const DUCK_T1: &[&str] = &[
    "0000011111000000","0000111111100000","0000111411110000","0000111111112200",
    "0000111111110000","0001111111111000","0011133111111000","0011133111110000",
    "0001111111100000","0000111111000000","0000111111000000","0000011110000000",
    "0000022200000000","0000202000000000","0000000000000000","0000000000000000",
];
const DUCK_T2: &[&str] = &[
    "0000011111000000","0000111111100000","0000111411100000","0000111111002200",
    "0000111111110000","0001111111111000","0011133111111000","0011133111110000",
    "0001111111100000","0000111111000000","0000111111000000","0000011110000000",
    "0000202000000000","0000022200000000","0000000000000000","0000000000000000",
];

fn get_frame(state: &AnimState, frame: usize) -> &'static [&'static str] {
    match state {
        AnimState::WalkRight => if frame % 2 == 0 { DUCK_R1 } else { DUCK_R2 },
        AnimState::WalkLeft  => if frame % 2 == 0 { DUCK_L1 } else { DUCK_L2 },
        AnimState::Thinking  => if frame % 2 == 0 { DUCK_T1 } else { DUCK_T2 },
        AnimState::Idle      => DUCK_R1,
    }
}

fn draw_duck(painter: &egui::Painter, pos: egui::Pos2, scale: f32, state: &AnimState, frame: usize) {
    let pixels = get_frame(state, frame);
    for (y, row) in pixels.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let idx = ch.to_digit(10).unwrap_or(0) as usize;
            if idx == 0 { continue; }
            if let Some(color) = PALETTE[idx] {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(pos.x + x as f32 * scale, pos.y + y as f32 * scale),
                    egui::vec2(scale, scale),
                );
                painter.rect_filled(rect, 0.0, color);
            }
        }
    }
}

impl AiosApp {
    fn new(db_path: String, conv_id: uuid::Uuid) -> Self {
        let (tx, rx) = mpsc::channel(256);
        Self {
            input:            String::new(),
            messages:         vec![ChatMsg {
                role:    Role::Assistant,
                content: "quack. ask me anything.".to_string(),
            }],
            chat_open:        false,
            duck_frame:       0,
            duck_x:           170.0,
            duck_y:           480.0,
            duck_vx:          60.0,
            frame_timer:      0.0,
            anim_state:       AnimState::WalkRight,
            db_path,
            conv_id,
            response_rx:      rx,
            response_tx:      tx,
            is_thinking:      false,
            current_response: String::new(),
            status_log:       Vec::new(),
        }
    }

    fn send_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() { return; }
        self.input.clear();

        self.messages.push(ChatMsg { role: Role::User, content: text.clone() });
        self.messages.push(ChatMsg { role: Role::Assistant, content: String::new() });
        self.is_thinking      = true;
        self.anim_state       = AnimState::Thinking;
        self.current_response = String::new();
        self.status_log.clear();

        let tx      = self.response_tx.clone();
        let db_path = self.db_path.clone();
        let conv_id = self.conv_id;

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt");

            rt.block_on(async move {
                tx.send(BackendMsg::Status("🧠 loading memories...".to_string())).await.ok();

                let db = match Database::open(&db_path) {
                    Ok(d)  => d,
                    Err(e) => {
                        tx.send(BackendMsg::Error(e.to_string())).await.ok();
                        return;
                    }
                };

                let assistant = match Assistant::with_defaults(db) {
                    Ok(a)  => a,
                    Err(e) => {
                        tx.send(BackendMsg::Error(e.to_string())).await.ok();
                        return;
                    }
                };

                tx.send(BackendMsg::Status("🔍 searching documents...".to_string())).await.ok();

                let (stream_tx, mut stream_rx) = mpsc::channel::<String>(256);
                let tx2 = tx.clone();
                tokio::spawn(async move {
                    while let Some(token) = stream_rx.recv().await {
                        tx2.send(BackendMsg::Token(token)).await.ok();
                    }
                });

                tx.send(BackendMsg::Status("💬 generating response...".to_string())).await.ok();

                match assistant.chat_stream_with_context(conv_id, &text, stream_tx).await {
                    Ok(_) => {
                        tx.send(BackendMsg::Status("✅ done.".to_string())).await.ok();
                        tx.send(BackendMsg::Done).await.ok();
                    }
                    Err(e) => {
                        tx.send(BackendMsg::Error(e.to_string())).await.ok();
                    }
                }
            });
        });
    }
}

impl eframe::App for AiosApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dt = ctx.input(|i| i.stable_dt).min(0.1);

        // Drain backend messages
        while let Ok(msg) = self.response_rx.try_recv() {
            match msg {
                BackendMsg::Token(t) => {
                    self.current_response.push_str(&t);
                    if let Some(last) = self.messages.last_mut() {
                        if last.role == Role::Assistant {
                            last.content = self.current_response.clone();
                        }
                    }
                }
                BackendMsg::Done => {
                    self.is_thinking = false;
                    self.anim_state  = if self.chat_open {
                        AnimState::Idle
                    } else {
                        AnimState::WalkRight
                    };
                }
                BackendMsg::Error(e) => {
                    self.is_thinking = false;
                    self.anim_state  = AnimState::WalkRight;
                    if let Some(last) = self.messages.last_mut() {
                        if last.role == Role::Assistant {
                            last.content = format!("Error: {}", e);
                        }
                    }
                }
                BackendMsg::Status(s) => {
                    self.status_log.push(s);
                    if self.status_log.len() > 5 {
                        self.status_log.remove(0);
                    }
                }
            }
        }

        // Animation timer
        let anim_speed = match self.anim_state {
            AnimState::Thinking  => 0.25,
            AnimState::WalkRight | AnimState::WalkLeft => 0.4,
            AnimState::Idle      => 0.8,
        };
        self.frame_timer += dt;
        if self.frame_timer >= anim_speed {
            self.frame_timer = 0.0;
            self.duck_frame  = self.duck_frame.wrapping_add(1);
        }

        // Duck roaming
        if !self.chat_open && self.anim_state != AnimState::Thinking {
            self.duck_x += self.duck_vx * dt;
            if self.duck_x + 64.0 >= 420.0 {
                self.duck_vx    = -60.0;
                self.anim_state = AnimState::WalkLeft;
            }
            if self.duck_x <= 0.0 {
                self.duck_vx    = 60.0;
                self.anim_state = AnimState::WalkRight;
            }
        }

        // Escape closes chat
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.chat_open {
            self.chat_open  = false;
            self.anim_state = AnimState::WalkRight;
        }

        let screen = ctx.screen_rect();

        // Transparent background
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |_ui| {});

        // Duck
        egui::Area::new(egui::Id::new("duck_area"))
            .fixed_pos(egui::pos2(self.duck_x, self.duck_y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(64.0, 64.0),
                    egui::Sense::click(),
                );
                draw_duck(ui.painter(), rect.min, 4.0, &self.anim_state, self.duck_frame);
                if response.clicked() {
                    self.chat_open  = !self.chat_open;
                    self.anim_state = if self.chat_open {
                        AnimState::Idle
                    } else {
                        AnimState::WalkRight
                    };
                    self.status_log.clear();
                }
            });

        // Chat panel
        if self.chat_open {
            let panel_w = 380.0_f32;
            let panel_h = 480.0_f32;
            let panel_x = (self.duck_x - panel_w + 64.0)
                .max(8.0)
                .min(screen.width() - panel_w - 8.0);
            let panel_y = (self.duck_y - panel_h - 8.0).max(8.0);

            egui::Window::new("aios_chat")
                .fixed_pos(egui::pos2(panel_x, panel_y))
                .fixed_size(egui::vec2(panel_w, panel_h))
                .title_bar(false)
                .resizable(false)
                .frame(egui::Frame {
                    fill:         egui::Color32::from_rgba_premultiplied(8, 8, 20, 245),
                    stroke:       egui::Stroke::new(1.0, egui::Color32::from_rgb(83, 74, 183)),
                    rounding:     egui::Rounding::same(14.0),
                    inner_margin: egui::Margin::same(12.0),
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    // Header
                    ui.horizontal(|ui| {
                        let dot_color = if self.is_thinking {
                            egui::Color32::from_rgb(239, 159, 39)
                        } else {
                            egui::Color32::from_rgb(29, 158, 117)
                        };
                        let dot_pos = ui.cursor().min + egui::vec2(5.0, 8.0);
                        ui.painter().circle_filled(dot_pos, 4.0, dot_color);
                        ui.add_space(14.0);
                        ui.colored_label(
                            egui::Color32::from_rgb(83, 74, 183),
                            if self.is_thinking { "Skvanchi — thinking..." } else { "Skvanchi — ready" },
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
                                self.chat_open  = false;
                                self.anim_state = AnimState::WalkRight;
                                self.status_log.clear();
                            }
                        });
                    });

                    ui.separator();

                    // Live action feed
                    if !self.status_log.is_empty() {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 60))
                            .rounding(egui::Rounding::same(6.0))
                            .inner_margin(egui::Margin::same(6.0))
                            .show(ui, |ui: &mut egui::Ui| {
                                for status in &self.status_log {
                                    ui.colored_label(
                                        egui::Color32::from_rgb(83, 74, 183),
                                        status,
                                    );
                                }
                            });
                        ui.add_space(4.0);
                    }

                    // Messages scroll area
                    let status_h = if self.status_log.is_empty() { 0.0 } else {
                        self.status_log.len() as f32 * 18.0 + 16.0
                    };
                    let msg_height = panel_h - 120.0 - status_h;

                    egui::ScrollArea::vertical()
                        .id_salt("msgs")
                        .max_height(msg_height)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            for msg in &self.messages {
                                if msg.content.is_empty() { continue; }
                                match msg.role {
                                    Role::User => {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::TOP),
                                            |ui| {
                                                egui::Frame::none()
                                                    .fill(egui::Color32::from_rgba_premultiplied(159, 225, 203, 20))
                                                    .rounding(egui::Rounding::same(8.0))
                                                    .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                                    .show(ui, |ui: &mut egui::Ui| {
                                                        ui.colored_label(
                                                            egui::Color32::from_rgb(159, 225, 203),
                                                            &msg.content,
                                                        );
                                                    });
                                            },
                                        );
                                    }
                                    Role::Assistant => {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label("🦆");
                                            egui::Frame::none()
                                                .fill(egui::Color32::from_rgba_premultiplied(83, 74, 183, 30))
                                                .rounding(egui::Rounding::same(8.0))
                                                .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                                                .show(ui, |ui: &mut egui::Ui| {
                                                    ui.colored_label(
                                                        egui::Color32::from_rgb(224, 224, 255),
                                                        &msg.content,
                                                    );
                                                });
                                        });
                                    }
                                }
                            }
                        });

                    ui.separator();

                    // Input row
                    ui.horizontal(|ui| {
                        let input = egui::TextEdit::singleline(&mut self.input)
                            .hint_text("ask anything...")
                            .desired_width(panel_w - 70.0);
                        let resp = ui.add(input);
                        if self.chat_open {
                            resp.request_focus();
                        }
                        let send = ui.button("→");
                        if send.clicked()
                            || (resp.lost_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                        {
                            self.send_message();
                        }
                    });
                });
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn main() -> eframe::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let data_dir = std::env::var("AIOS_DATA_DIR").unwrap_or_else(|_| {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| "C:\\Users\\user".to_string());
        format!("{}/.local/share/aios", home)
    });
    std::fs::create_dir_all(&data_dir).ok();
    let db_path = format!("{}/aios.db", data_dir);

    let conv_id = {
        let db        = Database::open(&db_path).expect("db open");
        let assistant = Assistant::with_defaults(db).expect("assistant");
        assistant.new_conversation(Some("desktop session")).expect("conv").id
    };

    info!("Starting Skvanchi");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_inner_size([420.0, 560.0])
            .with_position([1400.0, 480.0])
            .with_taskbar(false),
        ..Default::default()
    };

    eframe::run_native(
        "Skvanchi",
        options,
        Box::new(move |_cc| Ok(Box::new(AiosApp::new(db_path, conv_id)))),
    )
}