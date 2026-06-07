#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use egui::{Color32, Frame, Rounding, Stroke, Vec2, Visuals};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("IsoFlash")
            .with_inner_size([960.0, 620.0])
            .with_min_inner_size([700.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native("IsoFlash", options, Box::new(|cc| {
        let app = IsoFlash::default();
        app.apply_theme(&cc.egui_ctx);
        Box::new(app)
    }))
}

// ─── Enums ───────────────────────────────────────────────────────────────────

#[derive(Default, PartialEq, Clone, Debug)]
enum Panel {
    #[default] Dashboard,
    Catalogo, Descargas, Locales, Flasheo, Persistencia, Logs, Configuracion,
}

#[derive(Default, PartialEq, Clone)]
enum Tema { #[default] Oscuro, Claro }

// ─── USB ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct UsbDevice {
    name: String, model: String, size_bytes: u64, path: String, has_ventoy: bool,
}

// Acciones del dashboard: instalar ventoy, cancelar, flashear
enum DashAction { InstallVentoy(String), CancelVentoy, GoFlash(String) }

// ─── Cola de descargas ───────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum DownloadStatus { Queued, Downloading, Done, Error(String) }

#[derive(Clone)]
struct DownloadEntry {
    name: String,
    url:  String,
    size: String,
    status: DownloadStatus,
    progress: f32,
}

// ─── Catálogo ─────────────────────────────────────────────────────────────────

#[derive(Default, PartialEq, Clone, Debug)]
enum CatFilter { #[default] All, Rolling, Lts, Server, Security, Gaming, Windows }

#[derive(Clone)]
struct Distro {
    name: String, icon: String, description: String,
    category: CatFilter, size: String, arch: String,
    url: String, is_windows: bool,
}

fn build_catalog() -> Vec<Distro> {
    vec![
        Distro { name: "Arch Linux".into(),           icon: "🏹".into(), description: "Rolling release minimalista, para expertos".into(),            category: CatFilter::Rolling,   size: "~870 MB".into(),  arch: "x86_64".into(), url: "https://fastly.mirror.pkgbuild.com/iso/2026.06.01/archlinux-2026.06.01-x86_64.iso".into(),                                             is_windows: false },
        Distro { name: "Manjaro KDE".into(),           icon: "🟢".into(), description: "Arch con asistente gráfico, escritorio KDE Plasma".into(),     category: CatFilter::Rolling,   size: "~3.6 GB".into(),  arch: "x86_64".into(), url: "https://download.manjaro.org/kde/24.2.1/manjaro-kde-24.2.1-241217-linux612.iso".into(),                                                is_windows: false },
        Distro { name: "EndeavourOS".into(),           icon: "🚀".into(), description: "Arch con instalador amigable, mínimo bloat".into(),             category: CatFilter::Rolling,   size: "~1.8 GB".into(),  arch: "x86_64".into(), url: "https://mirror.alpix.eu/endeavouros/repo/EndeavourOS/x86_64/EndeavourOS_Endeavour-neo-2025.03.30.iso".into(),                             is_windows: false },
        Distro { name: "openSUSE Tumbleweed".into(),   icon: "🦎".into(), description: "Rolling enterprise-grade con herramientas YaST".into(),         category: CatFilter::Rolling,   size: "~1.1 GB".into(),  arch: "x86_64".into(), url: "https://download.opensuse.org/tumbleweed/iso/openSUSE-Tumbleweed-DVD-x86_64-Current.iso".into(),                                         is_windows: false },
        Distro { name: "Ubuntu 24.04 LTS".into(),      icon: "🟠".into(), description: "La distro más popular, soporte 5 años".into(),                  category: CatFilter::Lts,       size: "~5.7 GB".into(),  arch: "x86_64".into(), url: "https://releases.ubuntu.com/24.04/ubuntu-24.04.2-desktop-amd64.iso".into(),                                                            is_windows: false },
        Distro { name: "Linux Mint 22.1".into(),       icon: "🌿".into(), description: "Basada en Ubuntu, ideal para principiantes".into(),              category: CatFilter::Lts,       size: "~2.8 GB".into(),  arch: "x86_64".into(), url: "https://mirrors.layeronline.com/linuxmint/stable/22.1/linuxmint-22.1-cinnamon-64bit.iso".into(),                                          is_windows: false },
        Distro { name: "Debian 13 Trixie".into(),      icon: "🌀".into(), description: "Estable, universal, base de muchas distros".into(),              category: CatFilter::Lts,       size: "~700 MB".into(),  arch: "x86_64".into(), url: "https://cdimage.debian.org/cdimage/daily-builds/daily/arch-latest/amd64/iso-cd/debian-testing-amd64-netinst.iso".into(),                  is_windows: false },
        Distro { name: "Fedora 42".into(),             icon: "🎩".into(), description: "Innovadora, patrocinada por Red Hat, GNOME 47".into(),           category: CatFilter::Lts,       size: "~2.3 GB".into(),  arch: "x86_64".into(), url: "https://download.fedoraproject.org/pub/fedora/linux/releases/42/Workstation/x86_64/iso/Fedora-Workstation-Live-x86_64-42-1.1.iso".into(), is_windows: false },
        Distro { name: "antiX 23".into(),              icon: "🔷".into(), description: "Ligera, sin systemd, ideal para hardware viejo".into(),          category: CatFilter::Lts,       size: "~1.1 GB".into(),  arch: "x86_64".into(), url: "https://sourceforge.net/projects/antix-linux/files/Final/antiX-23/antiX-23_x64-full.iso".into(),                                         is_windows: false },
        Distro { name: "Alpine Linux 3.21".into(),     icon: "🏔".into(), description: "Mínima, segura, basada en musl y busybox".into(),                category: CatFilter::Lts,       size: "~230 MB".into(),  arch: "x86_64".into(), url: "https://dl-cdn.alpinelinux.org/alpine/v3.21/releases/x86_64/alpine-standard-3.21.3-x86_64.iso".into(),                                    is_windows: false },
        Distro { name: "Ubuntu Server 24.04".into(),   icon: "🖥".into(), description: "Servidor LTS, soporte cloud amplio".into(),                     category: CatFilter::Server,    size: "~2.6 GB".into(),  arch: "x86_64".into(), url: "https://releases.ubuntu.com/24.04/ubuntu-24.04.2-live-server-amd64.iso".into(),                                                       is_windows: false },
        Distro { name: "Debian 13 Netinst".into(),     icon: "🌐".into(), description: "Instalación mínima por red, ~700 MB".into(),                    category: CatFilter::Server,    size: "~700 MB".into(),  arch: "x86_64".into(), url: "https://cdimage.debian.org/cdimage/daily-builds/daily/arch-latest/amd64/iso-cd/debian-testing-amd64-netinst.iso".into(),                  is_windows: false },
        Distro { name: "AlmaLinux 10".into(),          icon: "🔴".into(), description: "Reemplazo CentOS, 100% compatible RHEL 10".into(),               category: CatFilter::Server,    size: "~1.8 GB".into(),  arch: "x86_64".into(), url: "https://repo.almalinux.org/almalinux/10/isos/x86_64/AlmaLinux-10-latest-x86_64-dvd.iso".into(),                                          is_windows: false },
        Distro { name: "Kali Linux 2025.1".into(),     icon: "🐉".into(), description: "Pentesting y hacking ético, +600 herramientas".into(),           category: CatFilter::Security,  size: "~3.9 GB".into(),  arch: "x86_64".into(), url: "https://cdimage.kali.org/current/kali-linux-2025.1a-installer-amd64.iso".into(),                                                        is_windows: false },
        Distro { name: "Tails 6.14".into(),            icon: "👻".into(), description: "Privacidad total, deja cero rastros".into(),                     category: CatFilter::Security,  size: "~1.5 GB".into(),  arch: "x86_64".into(), url: "https://download.tails.net/tails/stable/tails-amd64-6.14/tails-amd64-6.14.img".into(),                                                  is_windows: false },
        Distro { name: "ParrotOS 6.3".into(),          icon: "🦜".into(), description: "Seguridad y privacidad, ligero con MATE".into(),                  category: CatFilter::Security,  size: "~2.9 GB".into(),  arch: "x86_64".into(), url: "https://deb.parrot.sh/parrot/iso/6.3/Parrot-security-6.3_amd64.iso".into(),                                                            is_windows: false },
        Distro { name: "Nobara 41".into(),             icon: "🎮".into(), description: "Fedora optimizada para gaming con Proton patches".into(),         category: CatFilter::Gaming,    size: "~2.8 GB".into(),  arch: "x86_64".into(), url: "https://nobara-images.nobaraproject.org/Nobara-41-Official-2025-02-24.iso".into(),                                                     is_windows: false },
        Distro { name: "CachyOS".into(),               icon: "⚡".into(), description: "Arch optimizada, scheduler BORE, mejor rendimiento gaming".into(), category: CatFilter::Gaming,  size: "~2.6 GB".into(),  arch: "x86_64".into(), url: "https://mirror.cachyos.org/ISO/kde/latest/cachyos-kde-linux-latest.iso".into(),                                                         is_windows: false },
        Distro { name: "Bazzite".into(),               icon: "🕹".into(), description: "Gaming inmutable, base Fedora, Steam Deck ready".into(),          category: CatFilter::Gaming,    size: "~3.9 GB".into(),  arch: "x86_64".into(), url: "https://dl.bazzite.gg/Bazzite-latest-x86_64.iso".into(),                                                                              is_windows: false },
        Distro { name: "Windows 11".into(),            icon: "🪟".into(), description: "Requiere pasos adicionales — ver instrucciones".into(),           category: CatFilter::Windows,   size: "~5.4 GB".into(),  arch: "x86_64".into(), url: "https://www.microsoft.com/software-download/windows11".into(),                                                                        is_windows: true  },
        Distro { name: "Windows 10".into(),            icon: "🪟".into(), description: "Requiere pasos adicionales — ver instrucciones".into(),           category: CatFilter::Windows,   size: "~5.8 GB".into(),  arch: "x86_64".into(), url: "https://www.microsoft.com/software-download/windows10".into(),                                                                        is_windows: true  },
    ]
}

fn cat_badge(cat: &CatFilter, tema: &Tema) -> (Color32, Color32, &'static str) {
    match tema {
        Tema::Oscuro => match cat {
            CatFilter::All      => (Color32::from_rgb(40,40,60),   Color32::from_rgb(180,180,200), "Todas"),
            CatFilter::Rolling  => (Color32::from_rgb(60,30,90),   Color32::from_rgb(180,120,230), "Rolling"),
            CatFilter::Lts      => (Color32::from_rgb(20,60,40),   Color32::from_rgb(80,200,120),  "LTS"),
            CatFilter::Server   => (Color32::from_rgb(20,50,80),   Color32::from_rgb(80,160,220),  "Servidor"),
            CatFilter::Security => (Color32::from_rgb(80,30,30),   Color32::from_rgb(220,100,100), "Seguridad"),
            CatFilter::Gaming   => (Color32::from_rgb(80,50,20),   Color32::from_rgb(220,160,60),  "Gaming"),
            CatFilter::Windows  => (Color32::from_rgb(0,50,100),   Color32::from_rgb(80,160,240),  "Windows"),
        },
        Tema::Claro => match cat {
            CatFilter::All      => (Color32::from_rgb(220,220,235), Color32::from_rgb(70,70,110),   "Todas"),
            CatFilter::Rolling  => (Color32::from_rgb(230,215,245), Color32::from_rgb(110,50,170),  "Rolling"),
            CatFilter::Lts      => (Color32::from_rgb(210,240,220), Color32::from_rgb(30,130,70),   "LTS"),
            CatFilter::Server   => (Color32::from_rgb(210,230,245), Color32::from_rgb(30,100,170),  "Servidor"),
            CatFilter::Security => (Color32::from_rgb(250,220,220), Color32::from_rgb(180,40,40),   "Seguridad"),
            CatFilter::Gaming   => (Color32::from_rgb(250,235,205), Color32::from_rgb(160,100,10),  "Gaming"),
            CatFilter::Windows  => (Color32::from_rgb(210,230,255), Color32::from_rgb(20,80,190),   "Windows"),
        },
    }
}

// ─── Logs ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct LogEntry { timestamp: String, message: String, level: LogLevel }

#[derive(Clone, Debug, PartialEq)]
enum LogLevel { Info, Ok, Warn, Error }

#[derive(Default)]
struct OpProgress {
    label: String, progress: f32, active: bool,
    logs: Vec<LogEntry>, logs_expanded: bool,
    cancel_tx: Option<std::sync::mpsc::Sender<()>>,
}

impl OpProgress {
    fn add_log(&mut self, msg: &str, level: LogLevel) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        self.logs.push(LogEntry {
            timestamp: format!("{:02}:{:02}:{:02}", (secs/3600)%24, (secs/60)%60, secs%60),
            message: msg.to_string(), level,
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 { format!("{:.1} GB", bytes as f64 / 1_000_000_000.0) }
    else if bytes >= 1_000_000 { format!("{:.0} MB", bytes as f64 / 1_000_000.0) }
    else { format!("{} B", bytes) }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

// ─── USB scan ────────────────────────────────────────────────────────────────

fn scan_usbs() -> Vec<UsbDevice> {
    let output = match Command::new("lsblk")
        .args(["-J", "-b", "-o", "NAME,SIZE,MODEL,TRAN,TYPE"]).output()
    { Ok(o) if o.status.success() => o, _ => return vec![] };
    let v: serde_json::Value = match serde_json::from_str(
        &String::from_utf8_lossy(&output.stdout).to_string()
    ) { Ok(v) => v, Err(_) => return vec![] };
    let devices = match v["blockdevices"].as_array() { Some(d) => d, None => return vec![] };
    devices.iter().filter_map(|dev| {
        if dev["tran"].as_str().unwrap_or("") != "usb" || dev["type"].as_str().unwrap_or("") != "disk" { return None; }
        let name = dev["name"].as_str().unwrap_or("").to_string();
        let model = dev["model"].as_str().unwrap_or("USB Device").trim().to_string();
        let size_bytes = dev["size"].as_u64()
            .or_else(|| dev["size"].as_str().and_then(|s| s.parse().ok())).unwrap_or(0);
        let path = format!("/dev/{}", name);
        let has_ventoy = check_ventoy(&name);
        Some(UsbDevice { name, model, size_bytes, path, has_ventoy })
    }).collect()
}

fn check_ventoy(dev_name: &str) -> bool {
    Command::new("lsblk").args(["-o", "LABEL", &format!("/dev/{}", dev_name)]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("ventoy"))
        .unwrap_or(false)
}

// ─── Sidebar ─────────────────────────────────────────────────────────────────

fn sidebar_btn(
    ui: &mut egui::Ui, ctx: &egui::Context,
    panel: &mut Panel, tema: &Tema,
    target: Panel, icon: &str, label: &str,
    badge: bool,
) {
    let selected = *panel == target;
    let anim = ctx.animate_bool_with_time(egui::Id::new(format!("btn_{:?}", target)), selected, 0.18);
    let bg_base = match tema {
        Tema::Oscuro => Color32::from_rgb(18, 18, 26),
        Tema::Claro  => Color32::from_rgb(235, 237, 245),
    };
    let bg = lerp_color(bg_base, Color32::from_rgb(40, 80, 180), anim);
    let base_fg = match tema {
        Tema::Oscuro => Color32::from_rgb(180,185,200),
        Tema::Claro  => Color32::from_rgb(55,60,90),
    };
    let fg = lerp_color(base_fg, Color32::WHITE, anim);

    let resp = ui.add(
        egui::Button::new(egui::RichText::new(format!("{icon}  {label}")).size(14.0).color(fg))
            .fill(bg).rounding(Rounding::same(8.0)).min_size(Vec2::new(150.0, 38.0))
    );
    if resp.clicked() { *panel = target.clone(); }
    if anim > 0.01 && anim < 0.99 { ctx.request_repaint(); }

    // Punto rojo animado de notificación
    if badge {
        let t = ctx.input(|i| i.time) as f32;
        let pulse = ((t * 3.0).sin() * 0.3 + 0.7).clamp(0.0, 1.0);
        let badge_anim = ctx.animate_bool_with_time(
            egui::Id::new(format!("badge_{:?}", target)), badge, 0.35
        );
        if badge_anim > 0.01 {
            let dot_r = 5.0 * badge_anim;
            let pos = resp.rect.right_top() + egui::Vec2::new(-8.0, 8.0);
            let alpha = (pulse * badge_anim * 255.0) as u8;
            ui.painter().circle_filled(pos, dot_r, Color32::from_rgba_premultiplied(220, 50, 50, alpha));
            ui.painter().circle_stroke(pos, dot_r, Stroke::new(1.5, Color32::from_rgba_premultiplied(255,100,100, alpha)));
        }
        ctx.request_repaint();
    }
}

// ─── Dashboard ───────────────────────────────────────────────────────────────

fn draw_dashboard(
    ui: &mut egui::Ui, ctx: &egui::Context,
    usbs: &[UsbDevice], scanning: bool,
    op_active: bool, op_cancel_available: bool,
    tema: &Tema, action: &mut Option<DashAction>,
) {
    // Indicador de escaneo — solo visible durante el scan, sin contador regresivo
    if scanning {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.add_space(6.0);
            let col = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };
            ui.label(egui::RichText::new("Buscando dispositivos...").size(13.0).color(col));
        });
        ui.add_space(10.0);
    }

    if scanning && usbs.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(80.0); ui.spinner(); ui.add_space(12.0);
            let col = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(70,80,110) };
            ui.label(egui::RichText::new("Buscando dispositivos USB...").size(14.0).color(col));
        });
        return;
    }
    if usbs.is_empty() {
        let ico_col  = match tema { Tema::Oscuro => Color32::from_rgb(60,65,90),    Tema::Claro => Color32::from_rgb(150,160,195) };
        let txt_col  = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };
        let txt2_col = match tema { Tema::Oscuro => Color32::from_rgb(90,95,115),   Tema::Claro => Color32::from_rgb(110,120,150) };
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("💾").size(48.0).color(ico_col)); ui.add_space(12.0);
            ui.label(egui::RichText::new("No se encontraron dispositivos USB").size(15.0).color(txt_col)); ui.add_space(6.0);
            ui.label(egui::RichText::new("Se detectan automáticamente al conectar").size(12.0).color(txt2_col));
        });
        return;
    }

    let card_bg    = match tema { Tema::Oscuro => Color32::from_rgb(22,22,32),    Tema::Claro => Color32::WHITE };
    let border_col = match tema { Tema::Oscuro => Color32::from_rgb(40,44,60),    Tema::Claro => Color32::from_rgb(210,215,230) };
    let badge_bg   = match tema { Tema::Oscuro => Color32::from_rgb(30,35,55),    Tema::Claro => Color32::from_rgb(220,225,245) };
    let badge_fg   = match tema { Tema::Oscuro => Color32::from_rgb(180,190,220), Tema::Claro => Color32::from_rgb(60,70,120) };
    let path_col   = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(90,100,135) };

    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        for usb in usbs {
            let mut local: Option<DashAction> = None;
            Frame::none().fill(card_bg).rounding(Rounding::same(12.0))
                .stroke(Stroke::new(1.0, border_col)).inner_margin(16.0)
                .outer_margin(egui::Margin { left:0.0, right:0.0, top:0.0, bottom:12.0 })
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🔌").size(28.0)); ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&usb.model).size(15.0).strong());
                            ui.label(egui::RichText::new(&usb.path).size(12.0).color(path_col).monospace());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            Frame::none().fill(badge_bg).rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin { left:10.0, right:10.0, top:4.0, bottom:4.0 })
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(format_size(usb.size_bytes)).size(12.0).color(badge_fg));
                                });
                            ui.add_space(8.0);
                            let (vbg, vtxt, vfg) = if usb.has_ventoy {
                                (Color32::from_rgb(20,80,40), "✓ Ventoy", Color32::from_rgb(80,220,120))
                            } else {
                                match tema {
                                    Tema::Oscuro => (Color32::from_rgb(50,50,70), "Sin Ventoy", Color32::from_rgb(130,140,160)),
                                    Tema::Claro  => (Color32::from_rgb(220,220,235), "Sin Ventoy", Color32::from_rgb(100,105,140)),
                                }
                            };
                            Frame::none().fill(vbg).rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin { left:10.0, right:10.0, top:4.0, bottom:4.0 })
                                .show(ui, |ui| {
                                    ui.label(egui::RichText::new(vtxt).size(12.0).color(vfg));
                                });
                        });
                    });
                    ui.add_space(12.0); ui.separator(); ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if op_active {
                            // Spinner + label mientras hay operación
                            ui.spinner();
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Instalando Ventoy...").size(13.0).color(Color32::from_rgb(80,140,255)));
                            ui.add_space(8.0);
                            // Botón cancelar en el dashboard
                            if op_cancel_available {
                                if ui.add(egui::Button::new(egui::RichText::new("✕  Cancelar").size(12.0).color(Color32::from_rgb(220,80,80)))
                                    .fill(Color32::from_rgb(60,20,20)).rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(100.0, 30.0))).clicked() {
                                    local = Some(DashAction::CancelVentoy);
                                }
                            }
                        } else {
                            let vtxt = if usb.has_ventoy { "⬆  Actualizar Ventoy" } else { "⚡  Instalar Ventoy" };
                            if ui.add(egui::Button::new(egui::RichText::new(vtxt).size(13.0))
                                .fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(7.0))
                                .min_size(Vec2::new(165.0, 32.0))).clicked() {
                                local = Some(DashAction::InstallVentoy(usb.path.clone()));
                            }
                        }
                        ui.add_space(8.0);
                        if ui.add(egui::Button::new(egui::RichText::new("🔥  Flashear ISO").size(13.0))
                            .fill(Color32::from_rgb(160,60,20)).rounding(Rounding::same(7.0))
                            .min_size(Vec2::new(130.0, 32.0))).clicked() {
                            local = Some(DashAction::GoFlash(usb.path.clone()));
                        }
                    });
                });
            if local.is_some() { *action = local; }
        }
    });
}

// ─── Catálogo ─────────────────────────────────────────────────────────────────

fn draw_catalog(
    ui: &mut egui::Ui, catalog: &[Distro],
    search: &mut String, filter: &mut CatFilter,
    win_popup: &mut bool, win_name: &mut String,
    downloads: &mut Vec<DownloadEntry>,
    tema: &Tema,
) {
    ui.horizontal(|ui| {
        let search_w = (ui.available_width() - 130.0).max(320.0);
        ui.add(egui::TextEdit::singleline(search)
            .hint_text("🔍  Buscar distro...")
            .desired_width(search_w)
            .min_size(Vec2::new(0.0, 36.0))
            .font(egui::FontId::proportional(15.0)));
        if !search.is_empty() {
            if ui.add(egui::Button::new(egui::RichText::new("✕").size(14.0))
                .min_size(Vec2::new(32.0, 36.0))).clicked() { search.clear(); }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_space(4.0);
            let cnt_col = match tema { Tema::Oscuro => Color32::from_rgb(100,110,130), Tema::Claro => Color32::from_rgb(70,80,115) };
            ui.label(egui::RichText::new(format!("{} distros", catalog.len())).size(12.0).color(cnt_col));
        });
    });

    ui.add_space(12.0);

    ui.horizontal_wrapped(|ui| {
        let filters = [
            (CatFilter::All,      "🌐 Todas"),
            (CatFilter::Rolling,  "🔄 Rolling"),
            (CatFilter::Lts,      "🛡 LTS"),
            (CatFilter::Server,   "🖥 Servidor"),
            (CatFilter::Security, "🔐 Seguridad"),
            (CatFilter::Gaming,   "🎮 Gaming"),
            (CatFilter::Windows,  "🪟 Windows ⚠"),
        ];
        for (f, label) in &filters {
            let selected = *filter == *f;
            let bg = if selected { Color32::from_rgb(40,80,180) }
                else { match tema { Tema::Oscuro => Color32::from_rgb(25,25,38), Tema::Claro => Color32::from_rgb(220,222,235) } };
            let fg = if selected { Color32::WHITE }
                else { match tema { Tema::Oscuro => Color32::from_rgb(160,170,190), Tema::Claro => Color32::from_rgb(60,65,90) } };
            if ui.add(egui::Button::new(egui::RichText::new(*label).size(12.0).color(fg))
                .fill(bg).rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0, 26.0))).clicked() {
                *filter = f.clone();
            }
            ui.add_space(4.0);
        }
    });

    ui.add_space(16.0);

    let q = search.to_lowercase();
    let filtered: Vec<&Distro> = catalog.iter().filter(|d| {
        (*filter == CatFilter::All || d.category == *filter) &&
        (q.is_empty() || d.name.to_lowercase().contains(&q) || d.description.to_lowercase().contains(&q))
    }).collect();

    if filtered.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            let col = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(90,100,130) };
            ui.label(egui::RichText::new("Sin resultados para esa búsqueda").size(14.0).color(col));
        });
        return;
    }

    let card_bg    = match tema { Tema::Oscuro => Color32::from_rgb(22,22,32), Tema::Claro => Color32::WHITE };
    let border_col = match tema { Tema::Oscuro => Color32::from_rgb(40,44,60),  Tema::Claro => Color32::from_rgb(210,215,230) };
    let desc_col   = match tema { Tema::Oscuro => Color32::from_rgb(140,150,170), Tema::Claro => Color32::from_rgb(75,85,110) };
    let meta_col   = match tema { Tema::Oscuro => Color32::from_rgb(100,110,130), Tema::Claro => Color32::from_rgb(100,110,140) };
    let name_col   = match tema { Tema::Oscuro => Color32::WHITE, Tema::Claro => Color32::from_rgb(20,25,50) };

    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        let avail = ui.available_width();
        let card_w = ((avail - 16.0) / 2.0).max(260.0);

        for chunk in filtered.chunks(2) {
            ui.horizontal(|ui| {
                for distro in chunk {
                    ui.vertical(|ui| {
                        ui.set_width(card_w);
                        let mut clicked_download = false;

                        Frame::none().fill(card_bg).rounding(Rounding::same(12.0))
                            .stroke(Stroke::new(1.0, border_col)).inner_margin(14.0)
                            .show(ui, |ui| {
                                ui.set_min_width(card_w - 28.0);

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&distro.icon).size(26.0));
                                    ui.add_space(8.0);
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&distro.name).size(14.0).strong().color(name_col));
                                        let (cbg, cfg, ctxt) = cat_badge(&distro.category, tema);
                                        Frame::none().fill(cbg).rounding(Rounding::same(4.0))
                                            .inner_margin(egui::Margin { left:6.0, right:6.0, top:2.0, bottom:2.0 })
                                            .show(ui, |ui| {
                                                ui.label(egui::RichText::new(ctxt).size(10.0).color(cfg));
                                            });
                                    });
                                });

                                ui.add_space(8.0);
                                ui.label(egui::RichText::new(&distro.description).size(12.0).color(desc_col));
                                ui.add_space(8.0);

                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("💾 {}", distro.size)).size(11.0).color(meta_col));
                                    ui.add_space(10.0);
                                    ui.label(egui::RichText::new(format!("🔧 {}", distro.arch)).size(11.0).color(meta_col));
                                });

                                if distro.is_windows {
                                    ui.add_space(6.0);
                                    Frame::none().fill(Color32::from_rgb(60,40,10))
                                        .rounding(Rounding::same(6.0))
                                        .inner_margin(egui::Margin { left:8.0, right:8.0, top:5.0, bottom:5.0 })
                                        .show(ui, |ui| {
                                            ui.label(egui::RichText::new("⚠  Descarga especial requerida")
                                                .size(11.0).color(Color32::from_rgb(230,170,60)));
                                        });
                                }

                                ui.add_space(10.0); ui.separator(); ui.add_space(8.0);

                                let (btn_col, btn_txt) = if distro.is_windows {
                                    (Color32::from_rgb(0,90,190), "🪟  Ver instrucciones")
                                } else {
                                    // Si ya está en cola, mostrarlo
                                    let in_queue = downloads.iter().any(|d| d.url == distro.url);
                                    if in_queue {
                                        (Color32::from_rgb(30,80,40), "✓  En cola de descarga")
                                    } else {
                                        (Color32::from_rgb(40,80,180), "⬇  Descargar")
                                    }
                                };

                                if ui.add(egui::Button::new(egui::RichText::new(btn_txt).size(12.0))
                                    .fill(btn_col).rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(ui.available_width(), 30.0))).clicked() {
                                    clicked_download = true;
                                }
                            });

                        if clicked_download {
                            if distro.is_windows {
                                *win_popup = true;
                                *win_name  = distro.name.clone();
                            } else {
                                // Agregar a la cola de descargas en vez de abrir el navegador
                                let already = downloads.iter().any(|d| d.url == distro.url);
                                if !already {
                                    downloads.push(DownloadEntry {
                                        name:     distro.name.clone(),
                                        url:      distro.url.clone(),
                                        size:     distro.size.clone(),
                                        status:   DownloadStatus::Queued,
                                        progress: 0.0,
                                    });
                                }
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            });
            ui.add_space(10.0);
        }
    });
}

// ─── Descargas ───────────────────────────────────────────────────────────────

fn draw_descargas(ui: &mut egui::Ui, downloads: &mut Vec<DownloadEntry>, tema: &Tema) {
    let card_bg    = match tema { Tema::Oscuro => Color32::from_rgb(22,22,32),    Tema::Claro => Color32::WHITE };
    let border_col = match tema { Tema::Oscuro => Color32::from_rgb(40,44,60),    Tema::Claro => Color32::from_rgb(210,215,230) };
    let name_col   = match tema { Tema::Oscuro => Color32::WHITE,                 Tema::Claro => Color32::from_rgb(20,25,50) };
    let url_col    = match tema { Tema::Oscuro => Color32::from_rgb(100,110,130), Tema::Claro => Color32::from_rgb(90,100,135) };

    if downloads.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            let ic = match tema { Tema::Oscuro => Color32::from_rgb(60,65,90), Tema::Claro => Color32::from_rgb(150,160,195) };
            let tc = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };
            ui.label(egui::RichText::new("⬇").size(48.0).color(ic));
            ui.add_space(12.0);
            ui.label(egui::RichText::new("No hay descargas en cola").size(15.0).color(tc));
            ui.add_space(6.0);
            let tc2 = match tema { Tema::Oscuro => Color32::from_rgb(90,95,115), Tema::Claro => Color32::from_rgb(110,120,150) };
            ui.label(egui::RichText::new("Ve al Catálogo y presiona Descargar en una distro").size(12.0).color(tc2));
        });
        return;
    }

    ui.horizontal(|ui| {
        let tc = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };
        ui.label(egui::RichText::new(format!("{} elemento(s) en cola", downloads.len())).size(13.0).color(tc));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(egui::RichText::new("🗑  Limpiar completadas").size(12.0))
                .fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
                downloads.retain(|d| d.status != DownloadStatus::Done);
            }
        });
    });
    ui.add_space(10.0);

    let mut remove_idx: Option<usize> = None;
    let mut open_idx:   Option<usize> = None;

    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        for (i, dl) in downloads.iter().enumerate() {
            Frame::none().fill(card_bg).rounding(Rounding::same(10.0))
                .stroke(Stroke::new(1.0, border_col)).inner_margin(14.0)
                .outer_margin(egui::Margin { left:0.0, right:0.0, top:0.0, bottom:8.0 })
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let status_icon = match &dl.status {
                            DownloadStatus::Queued      => "🕐",
                            DownloadStatus::Downloading => "⬇",
                            DownloadStatus::Done        => "✅",
                            DownloadStatus::Error(_)    => "❌",
                        };
                        ui.label(egui::RichText::new(status_icon).size(20.0));
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&dl.name).size(14.0).strong().color(name_col));
                            ui.label(egui::RichText::new(format!("💾 {}  •  {}", dl.size, &dl.url[..dl.url.len().min(55)]))
                                .size(11.0).color(url_col).monospace());
                            if let DownloadStatus::Error(msg) = &dl.status {
                                ui.label(egui::RichText::new(format!("Error: {}", msg)).size(11.0).color(Color32::from_rgb(220,80,80)));
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new("✕").size(12.0))
                                .fill(Color32::TRANSPARENT)).clicked() {
                                remove_idx = Some(i);
                            }
                            if dl.status == DownloadStatus::Queued {
                                ui.add_space(6.0);
                                if ui.add(egui::Button::new(egui::RichText::new("▶  Descargar").size(12.0))
                                    .fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(6.0))
                                    .min_size(Vec2::new(100.0, 28.0))).clicked() {
                                    open_idx = Some(i);
                                }
                            }
                        });
                    });

                    // Barra de progreso si está descargando
                    if dl.status == DownloadStatus::Downloading && dl.progress > 0.0 {
                        ui.add_space(8.0);
                        let bar_w = ui.available_width() - 4.0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 6.0), egui::Sense::hover());
                        let bg = match tema { Tema::Oscuro => Color32::from_rgb(25,25,38), Tema::Claro => Color32::from_rgb(220,222,240) };
                        ui.painter().rect_filled(rect, Rounding::same(3.0), bg);
                        let fill_w = rect.width() * dl.progress;
                        let fill = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
                        ui.painter().rect_filled(fill, Rounding::same(3.0), Color32::from_rgb(40,100,220));
                    }
                });
        }
    });

    if let Some(i) = remove_idx { downloads.remove(i); }
    // Abrir URL en navegador (wget o aria2c para descarga real queda como trabajo futuro)
    if let Some(i) = open_idx {
        let _ = Command::new("xdg-open").arg(&downloads[i].url).spawn();
        downloads[i].status = DownloadStatus::Downloading;
    }
}

// ─── Logs ─────────────────────────────────────────────────────────────────────

fn draw_logs(ui: &mut egui::Ui, ctx: &egui::Context, op: &mut OpProgress, tema: &Tema) {
    let t = ctx.input(|i| i.time) as f32;

    if op.active {
        ui.add_space(8.0);
        let dots = ".".repeat((t * 2.0) as usize % 4);
        ui.label(egui::RichText::new(format!("⚡ {}{}", op.label, dots))
            .size(15.0).strong().color(Color32::from_rgb(80,140,255)));
        ui.add_space(12.0);

        let pct = (op.progress * 100.0) as u32;
        let bar_w = ui.available_width() - 20.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 28.0), egui::Sense::hover());
        let painter = ui.painter();
        let bar_bg = match tema { Tema::Oscuro => Color32::from_rgb(25,25,38), Tema::Claro => Color32::from_rgb(220,222,240) };
        painter.rect_filled(rect, Rounding::same(8.0), bar_bg);
        if op.progress > 0.0 {
            let fill_w = rect.width() * op.progress;
            let fill = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
            painter.rect_filled(fill, Rounding::same(8.0), Color32::from_rgb(30,80,200));
            let shine = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height() / 2.0));
            painter.rect_filled(shine, Rounding { nw:8.0, ne:8.0, sw:0.0, se:0.0 }, Color32::from_rgba_premultiplied(80,140,255,60));
        }
        let bar_brd = match tema { Tema::Oscuro => Color32::from_rgb(50,60,90), Tema::Claro => Color32::from_rgb(180,185,220) };
        painter.rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0, bar_brd));
        let pct_col = match tema { Tema::Oscuro => Color32::WHITE, Tema::Claro => Color32::from_rgb(20,30,70) };
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}%", pct), egui::FontId::proportional(13.0), pct_col);

        ui.add_space(12.0);
        if let Some(last) = op.logs.last() {
            let lc = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(90,100,130) };
            ui.label(egui::RichText::new(format!("  {}", last.message)).size(12.0).color(lc).italics());
        }
        ui.add_space(16.0);

        // Botón cancelar en logs
        if let Some(_tx) = &op.cancel_tx {
            if ui.add(egui::Button::new(egui::RichText::new("✕  Cancelar").size(13.0).color(Color32::from_rgb(220,80,80)))
                .fill(Color32::from_rgb(60,20,20)).rounding(Rounding::same(7.0))
                .min_size(Vec2::new(120.0, 30.0))).clicked() {
                let tx = op.cancel_tx.take().unwrap();
                let _ = tx.send(());
                op.active = false;
                op.add_log("Operación cancelada por el usuario", LogLevel::Warn);
            }
        }

    } else if !op.logs.is_empty() {
        ui.add_space(8.0);
        let ok  = op.logs.iter().any(|l| l.level == LogLevel::Ok);
        let err = op.logs.iter().any(|l| l.level == LogLevel::Error);
        let (icon, txt, col) = if ok && !err {
            ("✅", "Operación completada", Color32::from_rgb(80,200,120))
        } else if err {
            ("❌", "Operación con errores", Color32::from_rgb(220,80,80))
        } else {
            ("⚠", "Operación cancelada", Color32::from_rgb(220,180,60))
        };
        ui.label(egui::RichText::new(format!("{icon}  {txt}")).size(15.0).strong().color(col));
        ui.add_space(12.0);
    } else {
        let ic = match tema { Tema::Oscuro => Color32::from_rgb(60,65,90), Tema::Claro => Color32::from_rgb(150,160,195) };
        let tc = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("📋").size(40.0).color(ic)); ui.add_space(10.0);
            ui.label(egui::RichText::new("Sin operaciones activas").size(14.0).color(tc));
        });
        return;
    }

    if !op.logs.is_empty() {
        let toggle_txt = if op.logs_expanded { "▼  Ocultar logs detallados" } else { "▶  Ver logs detallados" };
        if ui.add(egui::Button::new(egui::RichText::new(toggle_txt).size(13.0).color(Color32::from_rgb(100,140,220)))
            .fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
            op.logs_expanded = !op.logs_expanded;
        }

        let anim = ctx.animate_bool_with_time(egui::Id::new("logs_expand"), op.logs_expanded, 0.20);
        if anim > 0.01 { ctx.request_repaint(); }

        if anim > 0.01 {
            ui.add_space(8.0);
            let log_bg  = match tema { Tema::Oscuro => Color32::from_rgb(12,12,18),    Tema::Claro => Color32::from_rgb(240,242,250) };
            let log_brd = match tema { Tema::Oscuro => Color32::from_rgb(40,44,65),    Tema::Claro => Color32::from_rgb(200,205,225) };
            let log_txt = match tema { Tema::Oscuro => Color32::from_rgb(200,205,220), Tema::Claro => Color32::from_rgb(40,45,70) };

            Frame::none().fill(log_bg).rounding(Rounding::same(10.0))
                .stroke(Stroke::new(1.0, log_brd)).inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width() - 20.0);
                    egui::ScrollArea::vertical().max_height(200.0 * anim).stick_to_bottom(true).show(ui, |ui| {
                        for entry in &op.logs {
                            let (prefix, col) = match entry.level {
                                LogLevel::Info  => ("INFO ", Color32::from_rgb(160,170,190)),
                                LogLevel::Ok    => ("OK   ", Color32::from_rgb(80,200,120)),
                                LogLevel::Warn  => ("WARN ", Color32::from_rgb(220,180,60)),
                                LogLevel::Error => ("ERR  ", Color32::from_rgb(220,80,80)),
                            };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("[{}]", entry.timestamp)).size(11.0).monospace().color(Color32::from_rgb(90,95,115)));
                                ui.label(egui::RichText::new(prefix).size(11.0).monospace().color(col));
                                ui.label(egui::RichText::new(&entry.message).size(11.0).monospace().color(log_txt));
                            });
                        }
                    });
                });
        }

        ui.add_space(12.0);
        if !op.active {
            if ui.add(egui::Button::new(egui::RichText::new("🗑  Limpiar logs").size(12.0).color(Color32::from_rgb(180,80,80)))
                .fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
                op.logs.clear(); op.logs_expanded = false;
            }
        }
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

struct IsoFlash {
    panel:      Panel,
    tema:       Tema,
    tema_anim:  f32,
    usbs:       Vec<UsbDevice>,
    scanning:   bool,
    last_scan:  f64,
    usb_rx:     Option<Receiver<Vec<UsbDevice>>>,
    op:         OpProgress,
    op_rx:      Option<Receiver<(f32, String, LogLevel, bool)>>,
    // Catálogo
    catalog:        Vec<Distro>,
    cat_search:     String,
    cat_filter:     CatFilter,
    cat_win_popup:  bool,
    cat_win_name:   String,
    // Cola de descargas
    downloads: Vec<DownloadEntry>,
}

impl Default for IsoFlash {
    fn default() -> Self {
        Self {
            panel: Panel::Dashboard, tema: Tema::Oscuro,
            tema_anim: 0.0,
            usbs: vec![], scanning: false,
            last_scan: -999.0,
            usb_rx: None,
            op: OpProgress::default(), op_rx: None,
            catalog: build_catalog(),
            cat_search: String::new(), cat_filter: CatFilter::All,
            cat_win_popup: false, cat_win_name: String::new(),
            downloads: vec![],
        }
    }
}

impl IsoFlash {
    fn start_install_ventoy(&mut self, path: String) {
        if self.op.active { return; }
        self.op = OpProgress::default();
        self.op.active = true;
        self.op.label = format!("Instalando Ventoy en {}", path);
        self.op.add_log(&format!("Iniciando instalación en {}", path), LogLevel::Info);

        let (tx, rx) = channel::<(f32, String, LogLevel, bool)>();
        let (cancel_tx, cancel_rx) = channel::<()>();
        self.op.cancel_tx = Some(cancel_tx);
        self.op_rx = Some(rx);

        std::thread::spawn(move || {
            let cancelled = || cancel_rx.try_recv().is_ok();
            let send = |p: f32, msg: &str, lvl: LogLevel, done: bool| {
                let _ = tx.send((p, msg.to_string(), lvl, done));
            };

            if cancelled() { send(0.0, "Cancelado", LogLevel::Warn, true); return; }

            send(0.05, &format!("Verificando dispositivo {}...", path), LogLevel::Info, false);
            match Command::new("lsblk").args([&path]).output() {
                Err(e) => { send(0.0, &format!("Error: {}", e), LogLevel::Error, true); return; }
                Ok(o) if !o.status.success() => {
                    send(0.0, &format!("Dispositivo {} no encontrado", path), LogLevel::Error, true); return;
                }
                _ => {}
            }

            send(0.10, "Leyendo tamaño del dispositivo...", LogLevel::Info, false);
            if let Ok(o) = Command::new("lsblk").args(["-b", "-n", "-o", "SIZE", &path]).output() {
                let size_str = String::from_utf8_lossy(&o.stdout).trim().to_string();
                // lsblk puede devolver varias líneas; tomar la primera
                let first = size_str.lines().next().unwrap_or("").trim();
                if let Ok(bytes) = first.parse::<u64>() {
                    let gb = bytes as f64 / 1_000_000_000.0;
                    send(0.15, &format!("Tamaño detectado: {:.1} GB", gb), LogLevel::Info, false);
                }
            }

            if cancelled() { send(0.0, "Cancelado", LogLevel::Warn, true); return; }

            // 1. Buscar ventoy instalado en el sistema
            send(0.20, "Buscando ventoy en el sistema...", LogLevel::Info, false);
            let ventoy_bin: Option<String> =
                if Command::new("which").arg("ventoy").output().map(|o| o.status.success()).unwrap_or(false) {
                    send(0.22, "ventoy encontrado en PATH", LogLevel::Info, false);
                    Some("ventoy".into())
                } else if std::path::Path::new("/opt/ventoy/Ventoy2Disk.sh").exists() {
                    send(0.22, "Ventoy encontrado en /opt/ventoy", LogLevel::Info, false);
                    Some("/opt/ventoy/Ventoy2Disk.sh".into())
                } else {
                    None
                };

            let bin = match ventoy_bin {
                Some(b) => b,
                None => {
                    // 2. Descargar ventoy al vuelo
                    send(0.25, "Ventoy no instalado. Descargando ventoy 1.1.12...", LogLevel::Warn, false);
                    let url = "https://github.com/ventoy/Ventoy/releases/download/v1.1.12/ventoy-1.1.12-linux.tar.gz";
                    let tmp_gz  = "/tmp/ventoy-isoflash.tar.gz";
                    let tmp_dir = "/tmp/ventoy-isoflash";
                    let script  = "/tmp/ventoy-isoflash/ventoy-1.1.12/Ventoy2Disk.sh";

                    // Limpiar descarga anterior si existe
                    let _ = std::fs::remove_file(tmp_gz);
                    let _ = std::fs::remove_dir_all(tmp_dir);

                    // Intentar wget, luego curl
                    let dl_ok = Command::new("wget")
                        .args(["-q", "--show-progress", "-O", tmp_gz, url])
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                        || Command::new("curl")
                            .args(["-L", "-o", tmp_gz, url])
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false);

                    if !dl_ok {
                        send(0.0, "Descarga fallida. Instala manualmente: paru -S ventoy", LogLevel::Error, true);
                        return;
                    }

                    send(0.40, "Descarga completa. Extrayendo...", LogLevel::Info, false);
                    let _ = std::fs::create_dir_all(tmp_dir);
                    let extract = Command::new("tar")
                        .args(["-xzf", tmp_gz, "-C", tmp_dir, "--strip-components=0"])
                        .output();

                    match extract {
                        Ok(o) if o.status.success() => {}
                        _ => {
                            send(0.0, "Error extrayendo el archivo descargado", LogLevel::Error, true);
                            return;
                        }
                    }

                    // Verificar que el script existe
                    if !std::path::Path::new(script).exists() {
                        // Listar para debug
                        if let Ok(o) = Command::new("find").args([tmp_dir, "-name", "Ventoy2Disk.sh"]).output() {
                            let found = String::from_utf8_lossy(&o.stdout);
                            let found = found.trim();
                            if !found.is_empty() {
                                send(0.45, &format!("Script encontrado en: {}", found), LogLevel::Info, false);
                                found.to_string()
                            } else {
                                send(0.0, "No se encontró Ventoy2Disk.sh en el paquete descargado", LogLevel::Error, true);
                                return;
                            }
                        } else {
                            send(0.0, "No se encontró Ventoy2Disk.sh en el paquete descargado", LogLevel::Error, true);
                            return;
                        }
                    } else {
                        script.to_string()
                    }
                }
            };

            if cancelled() { send(0.0, "Cancelado", LogLevel::Warn, true); return; }

            send(0.50, &format!("Usando: {}", bin), LogLevel::Info, false);
            send(0.55, "Ejecutando instalación (se pedirá contraseña)...", LogLevel::Warn, false);

            // Hacer ejecutable el script si es necesario
            if bin.ends_with(".sh") {
                let _ = Command::new("chmod").args(["+x", &bin]).output();
            }

            let result = Command::new("pkexec")
                .args(["bash", &bin, "-i", &path])
                .output();

            match result {
                Err(e) => { send(1.0, &format!("Error ejecutando pkexec: {}", e), LogLevel::Error, true); }
                Ok(o) => {
                    if o.status.success() {
                        let stdout = String::from_utf8_lossy(&o.stdout);
                        send(0.90, "Particionando y copiando archivos Ventoy...", LogLevel::Info, false);
                        for line in stdout.lines().take(4) {
                            let l = line.trim();
                            if !l.is_empty() { send(0.95, l, LogLevel::Info, false); }
                        }
                        send(1.0, "¡Ventoy instalado correctamente!", LogLevel::Ok, true);
                    } else {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        for line in stderr.lines().take(4) {
                            let l = line.trim();
                            if !l.is_empty() { send(0.0, l, LogLevel::Error, false); }
                        }
                        // pkexec code 127 = usuario canceló el diálogo de contraseña
                        if o.status.code() == Some(127) {
                            send(1.0, "Cancelado: no se proporcionó contraseña", LogLevel::Warn, true);
                        } else {
                            send(1.0, "Instalación fallida. Revisa los logs.", LogLevel::Error, true);
                        }
                    }
                }
            }
        });
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        match self.tema {
            Tema::Oscuro => {
                let mut vis = Visuals::dark();
                vis.panel_fill                             = Color32::from_rgb(15,15,20);
                vis.window_fill                            = Color32::from_rgb(20,20,28);
                vis.extreme_bg_color                       = Color32::from_rgb(10,10,14);
                vis.faint_bg_color                         = Color32::from_rgb(25,25,35);
                vis.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(200,205,220);
                vis.widgets.inactive.bg_fill               = Color32::from_rgb(30,30,42);
                vis.widgets.inactive.rounding              = Rounding::same(8.0);
                vis.widgets.inactive.fg_stroke.color       = Color32::from_rgb(180,185,200);
                vis.widgets.hovered.bg_fill                = Color32::from_rgb(50,100,200);
                vis.widgets.hovered.rounding               = Rounding::same(8.0);
                vis.widgets.active.bg_fill                 = Color32::from_rgb(40,80,180);
                vis.widgets.active.rounding                = Rounding::same(8.0);
                vis.selection.bg_fill                      = Color32::from_rgb(40,80,180);
                vis.override_text_color                    = None;
                ctx.set_visuals(vis);
            }
            Tema::Claro => {
                let mut vis = Visuals::light();
                vis.panel_fill                             = Color32::from_rgb(245,246,250);
                vis.window_fill                            = Color32::WHITE;
                vis.extreme_bg_color                       = Color32::from_rgb(230,232,240);
                vis.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(50,55,80);
                vis.widgets.noninteractive.bg_fill         = Color32::from_rgb(245,246,250);
                vis.widgets.inactive.bg_fill               = Color32::from_rgb(225,227,240);
                vis.widgets.inactive.rounding              = Rounding::same(8.0);
                vis.widgets.inactive.fg_stroke.color       = Color32::from_rgb(55,60,90);
                vis.widgets.hovered.bg_fill                = Color32::from_rgb(100,140,230);
                vis.widgets.hovered.rounding               = Rounding::same(8.0);
                vis.widgets.hovered.fg_stroke.color        = Color32::WHITE;
                vis.widgets.active.bg_fill                 = Color32::from_rgb(70,110,210);
                vis.widgets.active.rounding                = Rounding::same(8.0);
                vis.widgets.active.fg_stroke.color         = Color32::WHITE;
                vis.selection.bg_fill                      = Color32::from_rgb(70,110,210);
                vis.override_text_color                    = Some(Color32::from_rgb(25,30,55));
                ctx.set_visuals(vis);
            }
        }
    }
}

impl eframe::App for IsoFlash {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        // ── Auto-scan USB cada 2 segundos (silencioso, sin UI) ──
        let now = ctx.input(|i| i.time);
        if !self.scanning && (now - self.last_scan) >= 2.0 {
            self.last_scan = now;
            let (tx, rx) = channel();
            self.usb_rx = Some(rx);
            self.scanning = true;
            std::thread::spawn(move || { let _ = tx.send(scan_usbs()); });
        }
        if let Some(rx) = &self.usb_rx {
            if let Ok(usbs) = rx.try_recv() {
                self.usbs = usbs; self.scanning = false; self.usb_rx = None;
            }
        }

        // ── Recibir progreso de operación Ventoy ──
        if let Some(rx) = &self.op_rx {
            while let Ok((progress, msg, level, done)) = rx.try_recv() {
                if progress > 0.0 { self.op.progress = progress; }
                self.op.add_log(&msg, level);
                if done {
                    self.op.active = false;
                    self.op.cancel_tx = None;
                    self.op_rx = None;
                    break;
                }
            }
        }

        // ── Animación tema: lerp suave del color de fondo ──
        // En vez de un flash brusco, interpolamos panel_fill entre oscuro y claro
        let tema_target = match self.tema { Tema::Oscuro => 0.0_f32, Tema::Claro => 1.0_f32 };
        let speed = 0.08_f32;
        self.tema_anim = self.tema_anim + (tema_target - self.tema_anim) * speed;
        let anim_diff = (self.tema_anim - tema_target).abs();
        if anim_diff > 0.002 { ctx.request_repaint(); }

        // Interpolar el panel_fill en tiempo real para la transición suave
        let panel_dark  = Color32::from_rgb(15,15,20);
        let panel_light = Color32::from_rgb(245,246,250);
        let panel_now   = lerp_color(panel_dark, panel_light, self.tema_anim);
        let sidebar_dark  = Color32::from_rgb(18,18,26);
        let sidebar_light = Color32::from_rgb(235,237,245);
        let sidebar_now   = lerp_color(sidebar_dark, sidebar_light, self.tema_anim);

        // Aplicar el panel_fill interpolado sobre los visuals actuales
        {
            let mut vis = ctx.style().visuals.clone();
            vis.panel_fill = panel_now;
            ctx.set_visuals(vis);
        }

        // ── Repaint si hay operación activa ──
        if self.op.active { ctx.request_repaint(); }

        // Pulso logo
        let t = ctx.input(|i| i.time) as f32;
        let pulse = ((t * 1.8).sin() * 0.12 + 0.88).clamp(0.0, 1.0);
        let logo_color = Color32::from_rgb((80.0*pulse) as u8, (140.0*pulse) as u8, (255.0*pulse) as u8);
        // Request repaint para animación continua del logo
        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        let op_active           = self.op.active;
        let op_cancel_available = self.op.cancel_tx.is_some();

        // ── Sidebar ──
        egui::SidePanel::left("sidebar").exact_width(170.0)
            .frame(Frame::none().fill(sidebar_now).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.add_space(16.0);
                ui.label(egui::RichText::new("⚡ IsoFlash").size(20.0).strong().color(logo_color));
                ui.add_space(24.0);

                sidebar_btn(ui, ctx, &mut self.panel, &self.tema, Panel::Dashboard,    "🖥", "Dashboard",    false); ui.add_space(4.0);
                sidebar_btn(ui, ctx, &mut self.panel, &self.tema, Panel::Catalogo,     "📦", "Catálogo",     false); ui.add_space(4.0);
                sidebar_btn(ui, ctx, &mut self.panel, &self.tema, Panel::Descargas,    "⬇", "Descargas",    !self.downloads.is_empty()); ui.add_space(4.0);
                sidebar_btn(ui, ctx, &mut self.panel, &self.tema, Panel::Locales,      "💾", "ISOs Locales", false); ui.add_space(4.0);
                sidebar_btn(ui, ctx, &mut self.panel, &self.tema, Panel::Flasheo,      "🔥", "Flasheo",      false); ui.add_space(4.0);
                sidebar_btn(ui, ctx, &mut self.panel, &self.tema, Panel::Persistencia, "💿", "Persistencia", false); ui.add_space(4.0);
                sidebar_btn(ui, ctx, &mut self.panel, &self.tema, Panel::Logs,         "📋", "Logs",         op_active);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(8.0);
                    let (icon, label) = match self.tema { Tema::Oscuro => ("☀","Tema Claro"), Tema::Claro => ("🌙","Tema Oscuro") };
                    let toggle_col = match self.tema { Tema::Oscuro => Color32::from_rgb(180,185,200), Tema::Claro => Color32::from_rgb(60,65,90) };
                    if ui.add(egui::Button::new(egui::RichText::new(format!("{icon}  {label}")).size(13.0).color(toggle_col))
                        .fill(sidebar_now).rounding(Rounding::same(8.0)).min_size(Vec2::new(150.0,34.0))).clicked() {
                        self.tema = match self.tema { Tema::Oscuro => Tema::Claro, Tema::Claro => Tema::Oscuro };
                        self.apply_theme(ctx);
                    }
                    ui.add_space(4.0);
                    sidebar_btn(ui, ctx, &mut self.panel, &self.tema, Panel::Configuracion, "⚙", "Configuración", false);
                });
            });

        let mut dash_action: Option<DashAction> = None;

        // ── Panel central ──
        egui::CentralPanel::default()
            .frame(Frame::none().fill(panel_now).inner_margin(egui::Margin { left:20.0, right:20.0, top:0.0, bottom:0.0 }))
            .show(ctx, |ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    let (titulo, subtitulo) = match self.panel {
                        Panel::Dashboard =>     ("Dashboard",     "USBs conectados y estado Ventoy"),
                        Panel::Catalogo =>      ("Catálogo",      "Descarga ISOs verificadas"),
                        Panel::Descargas =>     ("Descargas",     "Cola de descargas activa"),
                        Panel::Locales =>       ("ISOs Locales",  "Gestiona tus ISOs descargadas"),
                        Panel::Flasheo =>       ("Flasheo",       "Escribe ISOs a tus USBs"),
                        Panel::Persistencia =>  ("Persistencia",  "Configura almacenamiento persistente"),
                        Panel::Logs =>          ("Logs",          "Progreso y detalles de operaciones"),
                        Panel::Configuracion => ("Configuración", "Ajustes de la aplicación"),
                    };
                    let fade = ctx.animate_value_with_time(egui::Id::new("panel_fade"), 1.0, 0.25);
                    let alpha = (fade * 255.0) as u8;
                    let sub_col = match self.tema {
                        Tema::Oscuro => Color32::from_rgba_premultiplied(130,140,160, alpha),
                        Tema::Claro  => Color32::from_rgba_premultiplied(75, 85,120,  alpha),
                    };
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(titulo).size(26.0).strong()
                            .color(Color32::from_rgba_premultiplied(60,120,240,alpha)));
                        ui.label(egui::RichText::new(subtitulo).size(13.0).color(sub_col));
                    });
                });
                ui.add_space(10.0); ui.separator(); ui.add_space(12.0);

                match self.panel {
                    Panel::Dashboard => {
                        draw_dashboard(ui, ctx, &self.usbs, self.scanning,
                            op_active, op_cancel_available, &self.tema, &mut dash_action);
                    }
                    Panel::Catalogo => {
                        draw_catalog(ui, &self.catalog, &mut self.cat_search, &mut self.cat_filter,
                            &mut self.cat_win_popup, &mut self.cat_win_name, &mut self.downloads, &self.tema);
                    }
                    Panel::Descargas => {
                        draw_descargas(ui, &mut self.downloads, &self.tema);
                    }
                    Panel::Logs => {
                        draw_logs(ui, ctx, &mut self.op, &self.tema);
                    }
                    _ => {
                        let col = match self.tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(100,110,140) };
                        ui.vertical_centered(|ui| {
                            ui.add_space(80.0);
                            ui.label(egui::RichText::new("🚧  En construcción").size(16.0).color(col));
                        });
                    }
                }
            });

        // ── Popup Windows ──
        if self.cat_win_popup {
            let win_url = if self.cat_win_name.contains("11") {
                "https://www.microsoft.com/software-download/windows11"
            } else {
                "https://www.microsoft.com/software-download/windows10"
            };
            egui::Window::new(format!("🪟  {} — Descarga especial", self.cat_win_name))
                .collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .fixed_size([440.0, 0.0])
                .show(ctx, |ui| {
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::from_rgb(60,40,10)).rounding(Rounding::same(8.0))
                        .inner_margin(12.0).show(ui, |ui| {
                            ui.label(egui::RichText::new("⚠  Windows no permite descarga directa de ISOs")
                                .size(13.0).strong().color(Color32::from_rgb(230,170,60)));
                        });
                    ui.add_space(12.0);
                    ui.label("Microsoft exige aceptar términos de licencia antes de entregar la ISO.");
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new("Pasos para obtener la ISO:").size(13.0).strong());
                    ui.add_space(6.0);
                    ui.label("1.  Visita el enlace oficial de Microsoft (abajo)");
                    ui.label("2.  Selecciona idioma y edición");
                    ui.label("3.  Acepta los términos y descarga la ISO");
                    ui.label("4.  Agrégala en ISOs Locales dentro de IsoFlash");
                    ui.add_space(12.0);
                    Frame::none()
                        .fill(match self.tema { Tema::Oscuro => Color32::from_rgb(20,20,30), Tema::Claro => Color32::from_rgb(235,238,250) })
                        .rounding(Rounding::same(6.0)).inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(win_url).size(11.0).monospace()
                                .color(Color32::from_rgb(80,160,240)));
                        });
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("Cerrar").size(13.0))
                            .fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(7.0))
                            .min_size(Vec2::new(100.0, 30.0))).clicked() {
                            self.cat_win_popup = false;
                        }
                    });
                    ui.add_space(4.0);
                });
        }

        // ── Acciones dashboard ──
        if let Some(action) = dash_action {
            match action {
                DashAction::InstallVentoy(path) => {
                    // Sin redirigir a Logs — el usuario decide cuándo ir a verlos
                    self.start_install_ventoy(path);
                }
                DashAction::CancelVentoy => {
                    if let Some(tx) = self.op.cancel_tx.take() {
                        let _ = tx.send(());
                    }
                    self.op.active = false;
                    self.op.add_log("Operación cancelada por el usuario", LogLevel::Warn);
                }
                DashAction::GoFlash(_path) => self.panel = Panel::Flasheo,
            }
        }
    }
}
