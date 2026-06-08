#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use egui::{Color32, Frame, Rounding, Stroke, Vec2, Visuals};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("IsoFlash")
            .with_inner_size([980.0, 640.0])
            .with_min_inner_size([720.0, 420.0]),
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
enum Panel { #[default] Dashboard, Catalogo, Descargas, Locales, Flasheo, Persistencia, Logs, Configuracion }

#[derive(Default, PartialEq, Clone)]
enum Tema { #[default] Oscuro, Claro }

#[derive(Default, PartialEq, Clone, Debug)]
enum CatFilter { #[default] All, Rolling, Lts, Server, Security, Gaming, Windows }

#[derive(Clone, Debug, PartialEq)]
enum DownloadStatus { Queued, Downloading, Done, Error(String) }

#[derive(Clone, Default, PartialEq)]
enum SpeedLimit { Low, Medium, High, #[default] Max }

impl SpeedLimit {
    fn rate_arg(&self) -> Option<&'static str> {
        match self { Self::Low => Some("500k"), Self::Medium => Some("2m"), Self::High => Some("8m"), Self::Max => None }
    }
    fn label(&self) -> &'static str {
        match self { Self::Low => "Baja  (~500 KB/s)", Self::Medium => "Media (~2 MB/s)", Self::High => "Alta  (~8 MB/s)", Self::Max => "Máxima (sin límite)" }
    }
}

// ─── Structs ─────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct UsbDevice { name: String, model: String, size_bytes: u64, path: String, has_ventoy: bool }

struct DlProgress { progress: f32, speed: String, done: bool, error: Option<String> }

struct DownloadEntry {
    name: String, url: String, display_size: String,
    total_bytes: u64, dest_path: String,
    status: DownloadStatus, progress: f32, speed_str: String,
    progress_rx: Option<Receiver<DlProgress>>,
}

#[derive(Clone)]
struct IsoFile { name: String, path: String, size_bytes: u64 }

#[derive(Clone)]
struct AppConfig { download_dir: String, speed_limit: SpeedLimit }

impl Default for AppConfig {
    fn default() -> Self { Self { download_dir: default_download_dir(), speed_limit: SpeedLimit::Max } }
}

enum DashAction { InstallVentoy(String, bool), CancelVentoy, GoFlash(String) }

enum DlAction { Start(usize), Remove(usize), OpenFile(usize), ClearDone }

// ─── Catálogo ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Distro {
    name: String, icon: String, logo: Option<&'static str>,
    description: String, category: CatFilter,
    size: String, arch: String, url: String, is_windows: bool,
}

fn d(name: &str, icon: &str, logo: Option<&'static str>, desc: &str, cat: CatFilter, size: &str, url: &str, win: bool) -> Distro {
    Distro { name: name.into(), icon: icon.into(), logo, description: desc.into(), category: cat, size: size.into(), arch: "x86_64".into(), url: url.into(), is_windows: win }
}

fn build_catalog() -> Vec<Distro> { vec![
    // ── Rolling ──────────────────────────────────────────────────────────────
    d("Arch Linux",           "🏹", Some("arch.png"),        "Rolling release minimalista, para expertos",              CatFilter::Rolling,   "~870 MB", "https://fastly.mirror.pkgbuild.com/iso/2026.06.01/archlinux-2026.06.01-x86_64.iso",                                                         false),
    d("Manjaro KDE",          "🟢", Some("manjaro.png"),     "Arch con asistente gráfico, escritorio KDE Plasma",        CatFilter::Rolling,   "~3.6 GB", "https://download.manjaro.org/kde/24.2.1/manjaro-kde-24.2.1-241217-linux612.iso",                                                             false),
    d("Manjaro GNOME",        "🟢", Some("manjaro.png"),     "Arch con GNOME, experiencia pulida y moderna",             CatFilter::Rolling,   "~3.4 GB", "https://download.manjaro.org/gnome/24.2.1/manjaro-gnome-24.2.1-241217-linux612.iso",                                                         false),
    d("EndeavourOS",          "🚀", Some("endeavouros.png"), "Arch con instalador amigable, mínimo bloat",               CatFilter::Rolling,   "~1.8 GB", "https://mirror.alpix.eu/endeavouros/repo/EndeavourOS/x86_64/EndeavourOS_Endeavour-neo-2025.03.30.iso",                                       false),
    d("openSUSE Tumbleweed",  "🦎", Some("opensuse.png"),    "Rolling enterprise-grade con herramientas YaST",           CatFilter::Rolling,   "~1.1 GB", "https://download.opensuse.org/tumbleweed/iso/openSUSE-Tumbleweed-DVD-x86_64-Current.iso",                                                     false),
    // ── LTS ──────────────────────────────────────────────────────────────────
    d("Ubuntu 24.04 LTS",     "🟠", Some("ubuntu.png"),      "La distro más popular, soporte 5 años, GNOME 46",          CatFilter::Lts,       "~5.7 GB", "https://releases.ubuntu.com/24.04/ubuntu-24.04.2-desktop-amd64.iso",                                                                          false),
    d("Linux Mint 22.1 Cinnamon", "🌿", Some("mint.png"),   "Mint con Cinnamon, familiar y elegante",                   CatFilter::Lts,       "~2.8 GB", "https://mirrors.layeronline.com/linuxmint/stable/22.1/linuxmint-22.1-cinnamon-64bit.iso",                                                     false),
    d("Linux Mint 22.1 XFCE", "🌿", Some("mint.png"),       "Mint con XFCE, ligero y rápido",                           CatFilter::Lts,       "~2.4 GB", "https://mirrors.layeronline.com/linuxmint/stable/22.1/linuxmint-22.1-xfce-64bit.iso",                                                          false),
    d("Linux Mint 22.1 MATE", "🌿", Some("mint.png"),       "Mint con MATE, clásico y estable",                         CatFilter::Lts,       "~2.5 GB", "https://mirrors.layeronline.com/linuxmint/stable/22.1/linuxmint-22.1-mate-64bit.iso",                                                          false),
    d("Debian 13 Trixie",     "🌀", Some("debian.png"),      "Estable, universal, base de muchas distros",               CatFilter::Lts,       "~700 MB", "https://cdimage.debian.org/cdimage/daily-builds/daily/arch-latest/amd64/iso-cd/debian-testing-amd64-netinst.iso",                             false),
    d("Fedora 44 Workstation","🎩", Some("fedora.png"),      "Innovadora, GNOME 50, Linux 6.19",                         CatFilter::Lts,       "~2.3 GB", "https://download.fedoraproject.org/pub/fedora/linux/releases/44/Workstation/x86_64/iso/Fedora-Workstation-Live-x86_64-44-1.1.iso",             false),
    d("Fedora 44 KDE",        "🎩", Some("fedora.png"),      "Fedora con KDE Plasma 6.6",                                CatFilter::Lts,       "~2.4 GB", "https://download.fedoraproject.org/pub/fedora/linux/releases/44/Spins/x86_64/iso/Fedora-KDE-Live-x86_64-44-1.1.iso",                           false),
    d("Fedora 44 Xfce",       "🎩", Some("fedora.png"),      "Fedora ligera con escritorio Xfce",                        CatFilter::Lts,       "~1.8 GB", "https://download.fedoraproject.org/pub/fedora/linux/releases/44/Spins/x86_64/iso/Fedora-Xfce-Live-x86_64-44-1.1.iso",                         false),
    d("antiX 23",             "🔷", Some("antix.png"),       "Ligera, sin systemd, ideal para hardware viejo",           CatFilter::Lts,       "~1.1 GB", "https://sourceforge.net/projects/antix-linux/files/Final/antiX-23/antiX-23_x64-full.iso",                                                     false),
    d("Alpine Linux 3.23.4",  "🏔", Some("alpine.png"),      "Mínima, segura, basada en musl y busybox",                 CatFilter::Lts,       "~230 MB", "https://dl-cdn.alpinelinux.org/alpine/v3.23/releases/x86_64/alpine-standard-3.23.4-x86_64.iso",                                               false),
    // ── Servidor ─────────────────────────────────────────────────────────────
    d("Ubuntu Server 24.04",  "🖥", Some("ubuntu.png"),      "Servidor LTS, soporte cloud amplio",                       CatFilter::Server,    "~2.6 GB", "https://releases.ubuntu.com/24.04/ubuntu-24.04.2-live-server-amd64.iso",                                                                      false),
    d("Debian 13 Netinst",    "🌐", Some("debian.png"),      "Instalación mínima por red, ~700 MB",                      CatFilter::Server,    "~700 MB", "https://cdimage.debian.org/cdimage/daily-builds/daily/arch-latest/amd64/iso-cd/debian-testing-amd64-netinst.iso",                             false),
    d("AlmaLinux 10",         "🔴", Some("almalinux.png"),   "Reemplazo CentOS, 100% compatible RHEL 10",                CatFilter::Server,    "~1.8 GB", "https://repo.almalinux.org/almalinux/10/isos/x86_64/AlmaLinux-10-latest-x86_64-dvd.iso",                                                      false),
    // ── Seguridad ────────────────────────────────────────────────────────────
    d("Kali Linux 2025.1",    "🐉", Some("kali.png"),        "Pentesting y hacking ético, +600 herramientas",            CatFilter::Security,  "~3.9 GB", "https://cdimage.kali.org/current/kali-linux-2025.1a-installer-amd64.iso",                                                                     false),
    d("Tails 6.14",           "👻", Some("tails.png"),       "Privacidad total, deja cero rastros",                      CatFilter::Security,  "~1.5 GB", "https://download.tails.net/tails/stable/tails-amd64-6.14/tails-amd64-6.14.img",                                                               false),
    d("ParrotOS 6.3",         "🦜", Some("parrot.png"),      "Seguridad y privacidad, ligero con MATE",                  CatFilter::Security,  "~2.9 GB", "https://deb.parrot.sh/parrot/iso/6.3/Parrot-security-6.3_amd64.iso",                                                                          false),
    // ── Gaming ───────────────────────────────────────────────────────────────
    d("Nobara 43",            "🎮", Some("nobara.png"),      "Fedora optimizada para gaming, Proton patches + OBS",      CatFilter::Gaming,    "~6.6 GB", "https://nobara-images.nobaraproject.org/Nobara-43-Official-2026-04-19.iso",                                                                    false),
    d("CachyOS",              "⚡", Some("cachyos.png"),     "Arch optimizada, scheduler BORE, mejor rendimiento",       CatFilter::Gaming,    "~2.6 GB", "https://mirror.cachyos.org/ISO/kde/latest/cachyos-kde-linux-latest.iso",                                                                       false),
    d("Bazzite",              "🕹", Some("bazzite.png"),     "Gaming inmutable, base Fedora, Steam Deck ready",          CatFilter::Gaming,    "~3.9 GB", "https://dl.bazzite.gg/Bazzite-latest-x86_64.iso",                                                                                              false),
    // ── Windows ──────────────────────────────────────────────────────────────
    d("Windows 11",           "🪟", Some("windows.png"),     "Requiere pasos adicionales — ver instrucciones",           CatFilter::Windows,   "~5.4 GB", "https://www.microsoft.com/software-download/windows11",                                                                                        true),
    d("Windows 10",           "🪟", Some("windows.png"),     "Requiere pasos adicionales — ver instrucciones",           CatFilter::Windows,   "~5.8 GB", "https://www.microsoft.com/software-download/windows10",                                                                                        true),
]}

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
        let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
        self.logs.push(LogEntry { timestamp: format!("{:02}:{:02}:{:02}", (s/3600)%24, (s/60)%60, s%60), message: msg.to_string(), level });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn format_size(bytes: u64) -> String {
    if bytes >= 1_000_000_000 { format!("{:.1} GB", bytes as f64 / 1e9) }
    else if bytes >= 1_000_000 { format!("{:.0} MB", bytes as f64 / 1e6) }
    else if bytes >= 1_000 { format!("{:.0} KB", bytes as f64 / 1e3) }
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

fn safe_filename(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn default_download_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let d = format!("{}/Descargas", home);
    if std::path::Path::new(&d).exists() { d } else { format!("{}/Downloads", home) }
}

fn logo_uri(file: &str) -> String {
    // Resolve relative to exe, fallback to cargo run from project root
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("assets").join("logos").join(file);
            if p.exists() { return format!("file://{}", p.display()); }
        }
    }
    format!("file://assets/logos/{}", file)
}

// ─── USB scan ────────────────────────────────────────────────────────────────

fn scan_usbs() -> Vec<UsbDevice> {
    let out = match Command::new("lsblk").args(["-J","-b","-o","NAME,SIZE,MODEL,TRAN,TYPE"]).output() {
        Ok(o) if o.status.success() => o, _ => return vec![],
    };
    let v: serde_json::Value = match serde_json::from_str(&String::from_utf8_lossy(&out.stdout)) {
        Ok(v) => v, Err(_) => return vec![],
    };
    v["blockdevices"].as_array().unwrap_or(&vec![]).iter().filter_map(|dev| {
        if dev["tran"].as_str().unwrap_or("") != "usb" || dev["type"].as_str().unwrap_or("") != "disk" { return None; }
        let name = dev["name"].as_str().unwrap_or("").to_string();
        let model = dev["model"].as_str().unwrap_or("USB Device").trim().to_string();
        let size_bytes = dev["size"].as_u64().or_else(|| dev["size"].as_str().and_then(|s| s.parse().ok())).unwrap_or(0);
        let path = format!("/dev/{}", name);
        let has_ventoy = check_ventoy(&name);
        Some(UsbDevice { name, model, size_bytes, path, has_ventoy })
    }).collect()
}

fn check_ventoy(dev_name: &str) -> bool {
    for flag in ["-o", "LABEL", "-o", "PARTLABEL"] {
        if Command::new("lsblk").args([flag, "LABEL", &format!("/dev/{}", dev_name)])
            .output().map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("ventoy")).unwrap_or(false) {
            return true;
        }
    }
    // Direct check via both LABEL and PARTLABEL in one call
    Command::new("lsblk").args(["-o", "LABEL,PARTLABEL", &format!("/dev/{}", dev_name)])
        .output().map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains("ventoy")).unwrap_or(false)
}

// Busca Ventoy2Disk.sh en el sistema completo. Retorna la ruta si se encuentra.
fn find_ventoy_bin(send: &dyn Fn(f32, &str, LogLevel, bool)) -> Option<String> {
    // 1. Buscar en PATH
    if Command::new("which").arg("ventoy").output().map(|o| o.status.success()).unwrap_or(false) {
        if let Ok(o) = Command::new("which").arg("ventoy").output() {
            let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !p.is_empty() {
                send(0.22, &format!("ventoy encontrado en PATH: {}", p), LogLevel::Info, false);
                return Some(p);
            }
        }
    }
    // 2. /opt/ventoy
    let opt = "/opt/ventoy/Ventoy2Disk.sh";
    if std::path::Path::new(opt).exists() {
        send(0.22, "Ventoy encontrado en /opt/ventoy", LogLevel::Info, false);
        return Some(opt.to_string());
    }
    // 3. Buscar en ~/Descargas y ~/Downloads (directorios ventoy-*)
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    for base in [format!("{}/Descargas", home), format!("{}/Downloads", home), home.clone()] {
        if let Ok(entries) = std::fs::read_dir(&base) {
            for e in entries.flatten() {
                let fname = e.file_name().to_string_lossy().to_lowercase();
                if fname.starts_with("ventoy") && e.path().is_dir() {
                    let script = e.path().join("Ventoy2Disk.sh");
                    if script.exists() {
                        let s = script.to_string_lossy().to_string();
                        send(0.23, &format!("Ventoy encontrado en: {}", s), LogLevel::Info, false);
                        return Some(s);
                    }
                }
            }
        }
    }
    // 4. find en home y /tmp (limitado a maxdepth 6)
    send(0.24, "Buscando Ventoy2Disk.sh en el sistema...", LogLevel::Info, false);
    for search_root in [home.as_str(), "/tmp", "/opt", "/usr/local"] {
        if let Ok(o) = Command::new("find")
            .args([search_root, "-name", "Ventoy2Disk.sh", "-maxdepth", "6"])
            .stderr(std::process::Stdio::null())
            .output()
        {
            let found = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if let Some(line) = found.lines().next() {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    send(0.25, &format!("Ventoy encontrado: {}", line), LogLevel::Info, false);
                    return Some(line);
                }
            }
        }
    }
    None
}

// ─── Descargas ────────────────────────────────────────────────────────────────

fn get_content_length(url: &str) -> u64 {
    if let Ok(o) = Command::new("curl").args(["-sIL", url]).output() {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let l = line.to_lowercase();
            if l.starts_with("content-length:") {
                if let Ok(n) = l.split(':').nth(1).unwrap_or("").trim().parse::<u64>() {
                    if n > 0 { return n; }
                }
            }
        }
    }
    0
}

fn start_download(entry: &mut DownloadEntry, config: &AppConfig) {
    if entry.status == DownloadStatus::Downloading { return; }
    let url   = entry.url.clone();
    let dest  = entry.dest_path.clone();
    let tmp   = format!("{}.part", dest);
    let rate  = config.speed_limit.rate_arg().map(|s| s.to_string());

    entry.status = DownloadStatus::Downloading;
    entry.progress = 0.0;
    entry.speed_str = "Conectando...".into();

    if let Some(parent) = std::path::Path::new(&dest).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let (tx, rx) = channel::<DlProgress>();
    entry.progress_rx = Some(rx);

    std::thread::spawn(move || {
        let total = get_content_length(&url);

        // Intentar wget primero, luego curl
        let mut args_w = vec!["-q", "-c", "-O", tmp.as_str(), url.as_str()];
        let rate_s: String;
        if let Some(r) = &rate { rate_s = r.clone(); args_w.extend(["--limit-rate", &rate_s]); }

        let child_res = Command::new("wget").args(&args_w).spawn()
            .or_else(|_| {
                let mut ac = vec!["-L", "-C", "-", "-o", tmp.as_str(), url.as_str()];
                if let Some(r) = &rate { ac.extend(["--limit-rate", r.as_str()]); }
                Command::new("curl").args(&ac).spawn()
            });

        let mut child = match child_res {
            Ok(c) => c,
            Err(e) => { let _ = tx.send(DlProgress { progress:0.0, speed:String::new(), done:true, error:Some(format!("Error: {}", e)) }); return; }
        };

        let mut last_bytes = 0u64;
        let mut last_tick  = Instant::now();

        loop {
            std::thread::sleep(Duration::from_millis(800));
            let current = std::fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0);
            let dt = last_tick.elapsed().as_secs_f64().max(0.1);
            let speed = ((current.saturating_sub(last_bytes)) as f64 / dt) as u64;
            let progress = if total > 0 { (current as f32 / total as f32).min(0.99) } else { 0.0 };
            let _ = tx.send(DlProgress { progress, speed: format!("{}/s", format_size(speed)), done: false, error: None });
            last_bytes = current; last_tick = Instant::now();
            match child.try_wait() {
                Ok(Some(s)) => {
                    if s.success() {
                        let _ = std::fs::rename(&tmp, &dest);
                        let _ = tx.send(DlProgress { progress:1.0, speed:String::new(), done:true, error:None });
                    } else {
                        let _ = tx.send(DlProgress { progress:0.0, speed:String::new(), done:true, error:Some("Descarga fallida".into()) });
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => { let _ = tx.send(DlProgress { progress:0.0, speed:String::new(), done:true, error:Some(e.to_string()) }); break; }
            }
        }
    });
}

fn scan_iso_files(dir: &str) -> Vec<IsoFile> {
    std::fs::read_dir(dir).ok()
        .into_iter().flatten()
        .flatten()
        .filter(|e| {
            let ext = e.path().extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
            ext == "iso" || ext == "img"
        })
        .map(|e| IsoFile {
            name: e.file_name().to_string_lossy().to_string(),
            path: e.path().to_string_lossy().to_string(),
            size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
        })
        .collect()
}

// ─── Sidebar ─────────────────────────────────────────────────────────────────

fn sidebar_btn(ui: &mut egui::Ui, ctx: &egui::Context, panel: &mut Panel, tema: &Tema, target: Panel, icon: &str, label: &str, badge: bool) {
    let selected = *panel == target;
    let anim = ctx.animate_bool_with_time(egui::Id::new(format!("btn_{:?}", target)), selected, 0.18);
    let bg_base = match tema { Tema::Oscuro => Color32::from_rgb(18,18,26), Tema::Claro => Color32::from_rgb(235,237,245) };
    let bg = lerp_color(bg_base, Color32::from_rgb(40,80,180), anim);
    let base_fg = match tema { Tema::Oscuro => Color32::from_rgb(180,185,200), Tema::Claro => Color32::from_rgb(55,60,90) };
    let fg = lerp_color(base_fg, Color32::WHITE, anim);
    let resp = ui.add(egui::Button::new(egui::RichText::new(format!("{icon}  {label}")).size(14.0).color(fg))
        .fill(bg).rounding(Rounding::same(8.0)).min_size(Vec2::new(150.0, 38.0)));
    if resp.clicked() { *panel = target.clone(); }
    if anim > 0.01 && anim < 0.99 { ctx.request_repaint(); }
    if badge {
        let t = ctx.input(|i| i.time) as f32;
        let pulse = ((t * 3.0).sin() * 0.3 + 0.7).clamp(0.0, 1.0);
        let ba = ctx.animate_bool_with_time(egui::Id::new(format!("badge_{:?}", target)), badge, 0.35);
        if ba > 0.01 {
            let pos = resp.rect.right_top() + egui::vec2(-8.0, 8.0);
            let alpha = (pulse * ba * 255.0) as u8;
            ui.painter().circle_filled(pos, 5.0 * ba, Color32::from_rgba_premultiplied(220,50,50,alpha));
            ui.painter().circle_stroke(pos, 5.0 * ba, Stroke::new(1.5, Color32::from_rgba_premultiplied(255,100,100,alpha)));
        }
        ctx.request_repaint();
    }
}

// ─── Dashboard ───────────────────────────────────────────────────────────────

fn draw_dashboard(ui: &mut egui::Ui, _ctx: &egui::Context, usbs: &[UsbDevice], scanning: bool, op_active: bool, op_cancel: bool, tema: &Tema, action: &mut Option<DashAction>) {
    // Estado vacío — el spinner solo sale cuando NO hay dispositivos todavía
    if usbs.is_empty() {
        let ic = match tema { Tema::Oscuro => Color32::from_rgb(60,65,90),    Tema::Claro => Color32::from_rgb(150,160,195) };
        let tc = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };
        let t2 = match tema { Tema::Oscuro => Color32::from_rgb(90,95,115),   Tema::Claro => Color32::from_rgb(110,120,150) };
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            if scanning { ui.spinner(); } else { ui.label(egui::RichText::new("💾").size(48.0).color(ic)); }
            ui.add_space(12.0);
            ui.label(egui::RichText::new(if scanning { "Buscando dispositivos..." } else { "No se encontraron dispositivos USB" }).size(15.0).color(tc));
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Se detectan automáticamente al conectar").size(12.0).color(t2));
        });
        return;
    }

    // Lista de USBs — NO muestra nada del scan en curso, la lista simplemente se actualiza
    let card_bg    = match tema { Tema::Oscuro => Color32::from_rgb(22,22,32),    Tema::Claro => Color32::WHITE };
    let border_col = match tema { Tema::Oscuro => Color32::from_rgb(40,44,60),    Tema::Claro => Color32::from_rgb(210,215,230) };
    let badge_bg   = match tema { Tema::Oscuro => Color32::from_rgb(30,35,55),    Tema::Claro => Color32::from_rgb(220,225,245) };
    let badge_fg   = match tema { Tema::Oscuro => Color32::from_rgb(180,190,220), Tema::Claro => Color32::from_rgb(60,70,120) };
    let path_col   = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(90,100,135) };
    let name_col   = match tema { Tema::Oscuro => Color32::WHITE,                 Tema::Claro => Color32::from_rgb(20,25,50) };

    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        for usb in usbs {
            let mut local: Option<DashAction> = None;
            Frame::none().fill(card_bg).rounding(Rounding::same(12.0))
                .stroke(Stroke::new(1.0, border_col)).inner_margin(16.0)
                .outer_margin(egui::Margin { left:0.0, right:0.0, top:0.0, bottom:12.0 })
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    // Cabecera: icono + modelo + tamaño + badge ventoy
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🔌").size(28.0)); ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&usb.model).size(15.0).strong().color(name_col));
                            ui.label(egui::RichText::new(&usb.path).size(12.0).color(path_col).monospace());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            Frame::none().fill(badge_bg).rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin { left:10.0, right:10.0, top:4.0, bottom:4.0 })
                                .show(ui, |ui| { ui.label(egui::RichText::new(format_size(usb.size_bytes)).size(12.0).color(badge_fg)); });
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
                                .show(ui, |ui| { ui.label(egui::RichText::new(vtxt).size(12.0).color(vfg)); });
                        });
                    });
                    ui.add_space(12.0); ui.separator(); ui.add_space(10.0);
                    // Botones de acción
                    ui.horizontal(|ui| {
                        if op_active {
                            ui.spinner(); ui.add_space(6.0);
                            ui.label(egui::RichText::new("Instalando Ventoy...").size(13.0).color(Color32::from_rgb(80,140,255)));
                            ui.add_space(8.0);
                            if op_cancel {
                                if ui.add(egui::Button::new(egui::RichText::new("✕  Cancelar").size(12.0).color(Color32::from_rgb(220,80,80)))
                                    .fill(Color32::from_rgb(60,20,20)).rounding(Rounding::same(7.0)).min_size(Vec2::new(100.0,30.0))).clicked() {
                                    local = Some(DashAction::CancelVentoy);
                                }
                            }
                        } else {
                            let (vtxt, is_update) = if usb.has_ventoy { ("⬆  Actualizar Ventoy", true) } else { ("⚡  Instalar Ventoy", false) };
                            if ui.add(egui::Button::new(egui::RichText::new(vtxt).size(13.0).color(Color32::WHITE))
                                .fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(7.0)).min_size(Vec2::new(165.0,32.0))).clicked() {
                                local = Some(DashAction::InstallVentoy(usb.path.clone(), is_update));
                            }
                        }
                        ui.add_space(8.0);
                        if ui.add(egui::Button::new(egui::RichText::new("🔥  Flashear ISO").size(13.0).color(Color32::WHITE))
                            .fill(Color32::from_rgb(160,60,20)).rounding(Rounding::same(7.0)).min_size(Vec2::new(130.0,32.0))).clicked() {
                            local = Some(DashAction::GoFlash(usb.path.clone()));
                        }
                    });
                });
            if local.is_some() { *action = local; }
        }
    });
}

// ─── Catálogo ─────────────────────────────────────────────────────────────────

fn draw_catalog(ui: &mut egui::Ui, catalog: &[Distro], search: &mut String, filter: &mut CatFilter, win_popup: &mut bool, win_name: &mut String, downloads: &mut Vec<DownloadEntry>, config: &AppConfig, tema: &Tema) {
    ui.horizontal(|ui| {
        let sw = (ui.available_width() - 130.0).max(320.0);
        ui.add(egui::TextEdit::singleline(search).hint_text("🔍  Buscar distro...").desired_width(sw).min_size(Vec2::new(0.0,36.0)).font(egui::FontId::proportional(15.0)));
        if !search.is_empty() {
            if ui.add(egui::Button::new(egui::RichText::new("✕").size(14.0)).min_size(Vec2::new(32.0,36.0))).clicked() { search.clear(); }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let cc = match tema { Tema::Oscuro => Color32::from_rgb(100,110,130), Tema::Claro => Color32::from_rgb(70,80,115) };
            ui.label(egui::RichText::new(format!("{} distros", catalog.len())).size(12.0).color(cc));
        });
    });
    ui.add_space(12.0);
    ui.horizontal_wrapped(|ui| {
        for (f, label) in &[(CatFilter::All,"🌐 Todas"),(CatFilter::Rolling,"🔄 Rolling"),(CatFilter::Lts,"🛡 LTS"),(CatFilter::Server,"🖥 Servidor"),(CatFilter::Security,"🔐 Seguridad"),(CatFilter::Gaming,"🎮 Gaming"),(CatFilter::Windows,"🪟 Windows ⚠")] {
            let sel = *filter == *f;
            let bg = if sel { Color32::from_rgb(40,80,180) } else { match tema { Tema::Oscuro => Color32::from_rgb(25,25,38), Tema::Claro => Color32::from_rgb(220,222,235) } };
            let fg = if sel { Color32::WHITE } else { match tema { Tema::Oscuro => Color32::from_rgb(160,170,190), Tema::Claro => Color32::from_rgb(60,65,90) } };
            if ui.add(egui::Button::new(egui::RichText::new(*label).size(12.0).color(fg)).fill(bg).rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0,26.0))).clicked() { *filter = f.clone(); }
            ui.add_space(4.0);
        }
    });
    ui.add_space(16.0);
    let q = search.to_lowercase();
    let filtered: Vec<&Distro> = catalog.iter().filter(|d|
        (*filter == CatFilter::All || d.category == *filter) &&
        (q.is_empty() || d.name.to_lowercase().contains(&q) || d.description.to_lowercase().contains(&q))
    ).collect();
    if filtered.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            let c = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(90,100,130) };
            ui.label(egui::RichText::new("Sin resultados").size(14.0).color(c));
        }); return;
    }
    let card_bg  = match tema { Tema::Oscuro => Color32::from_rgb(22,22,32), Tema::Claro => Color32::WHITE };
    let brd      = match tema { Tema::Oscuro => Color32::from_rgb(40,44,60),    Tema::Claro => Color32::from_rgb(210,215,230) };
    let desc_col = match tema { Tema::Oscuro => Color32::from_rgb(140,150,170), Tema::Claro => Color32::from_rgb(75,85,110) };
    let meta_col = match tema { Tema::Oscuro => Color32::from_rgb(100,110,130), Tema::Claro => Color32::from_rgb(100,110,140) };
    let name_col = match tema { Tema::Oscuro => Color32::WHITE, Tema::Claro => Color32::from_rgb(20,25,50) };

    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        let avail = ui.available_width();
        let card_w = ((avail - 16.0) / 2.0).max(260.0);
        for chunk in filtered.chunks(2) {
            ui.horizontal(|ui| {
                for distro in chunk {
                    ui.vertical(|ui| {
                        ui.set_width(card_w);
                        let mut clicked = false;
                        Frame::none().fill(card_bg).rounding(Rounding::same(12.0)).stroke(Stroke::new(1.0,brd)).inner_margin(14.0).show(ui, |ui| {
                            ui.set_min_width(card_w - 28.0);
                            ui.horizontal(|ui| {
                                // Logo o emoji
                                if let Some(logo) = distro.logo {
                                    let uri = logo_uri(logo);
                                    ui.add(egui::Image::new(uri.as_str()).max_size(Vec2::new(32.0,32.0)).rounding(Rounding::same(4.0)));
                                } else {
                                    ui.label(egui::RichText::new(&distro.icon).size(26.0));
                                }
                                ui.add_space(8.0);
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(&distro.name).size(14.0).strong().color(name_col));
                                    let (cbg,cfg,ctxt) = cat_badge(&distro.category, tema);
                                    Frame::none().fill(cbg).rounding(Rounding::same(4.0)).inner_margin(egui::Margin{left:6.0,right:6.0,top:2.0,bottom:2.0}).show(ui, |ui| {
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
                                Frame::none().fill(Color32::from_rgb(60,40,10)).rounding(Rounding::same(6.0)).inner_margin(egui::Margin{left:8.0,right:8.0,top:5.0,bottom:5.0}).show(ui, |ui| {
                                    ui.label(egui::RichText::new("⚠  Descarga especial requerida").size(11.0).color(Color32::from_rgb(230,170,60)));
                                });
                            }
                            ui.add_space(10.0); ui.separator(); ui.add_space(8.0);
                            let in_queue = downloads.iter().any(|d| d.url == distro.url);
                            let (btn_col, btn_txt) = if distro.is_windows {
                                (Color32::from_rgb(0,90,190), "🪟  Ver instrucciones")
                            } else if in_queue {
                                (Color32::from_rgb(30,80,40), "✓  En cola de descarga")
                            } else {
                                (Color32::from_rgb(40,80,180), "⬇  Agregar a descargas")
                            };
                            if ui.add(egui::Button::new(egui::RichText::new(btn_txt).size(12.0).color(Color32::WHITE)).fill(btn_col).rounding(Rounding::same(7.0)).min_size(Vec2::new(ui.available_width(),30.0))).clicked() {
                                clicked = true;
                            }
                        });
                        if clicked {
                            if distro.is_windows { *win_popup = true; *win_name = distro.name.clone(); }
                            else if !downloads.iter().any(|d| d.url == distro.url) {
                                downloads.push(DownloadEntry {
                                    name: distro.name.clone(), url: distro.url.clone(),
                                    display_size: distro.size.clone(), total_bytes: 0,
                                    dest_path: format!("{}/{}.iso", config.download_dir, safe_filename(&distro.name)),
                                    status: DownloadStatus::Queued, progress: 0.0, speed_str: String::new(), progress_rx: None,
                                });
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

fn draw_descargas(ui: &mut egui::Ui, _ctx: &egui::Context, downloads: &mut Vec<DownloadEntry>, config: &AppConfig, tema: &Tema) -> Option<DlAction> {
    let card_bg  = match tema { Tema::Oscuro => Color32::from_rgb(22,22,32),    Tema::Claro => Color32::WHITE };
    let brd      = match tema { Tema::Oscuro => Color32::from_rgb(40,44,60),    Tema::Claro => Color32::from_rgb(210,215,230) };
    let name_col = match tema { Tema::Oscuro => Color32::WHITE,                 Tema::Claro => Color32::from_rgb(20,25,50) };
    let url_col  = match tema { Tema::Oscuro => Color32::from_rgb(100,110,130), Tema::Claro => Color32::from_rgb(90,100,135) };
    let tc       = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };

    if downloads.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            let ic = match tema { Tema::Oscuro => Color32::from_rgb(60,65,90), Tema::Claro => Color32::from_rgb(150,160,195) };
            ui.label(egui::RichText::new("⬇").size(48.0).color(ic)); ui.add_space(12.0);
            ui.label(egui::RichText::new("No hay descargas en cola").size(15.0).color(tc)); ui.add_space(6.0);
            let t2 = match tema { Tema::Oscuro => Color32::from_rgb(90,95,115), Tema::Claro => Color32::from_rgb(110,120,150) };
            ui.label(egui::RichText::new("Ve al Catálogo y pulsa «Agregar a descargas»").size(12.0).color(t2));
        });
        return None;
    }

    // Info bar: dir + velocidad
    let bar_bg = match tema { Tema::Oscuro => Color32::from_rgb(18,18,28), Tema::Claro => Color32::from_rgb(230,232,245) };
    Frame::none().fill(bar_bg).rounding(Rounding::same(8.0)).inner_margin(10.0).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("📁").size(13.0));
            ui.label(egui::RichText::new(&config.download_dir).size(12.0).color(url_col).monospace());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("⚡ {}", config.speed_limit.label())).size(12.0).color(tc));
            });
        });
    });
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} elemento(s)", downloads.len())).size(13.0).color(tc));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(egui::RichText::new("🗑  Limpiar completadas").size(12.0)).fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
                return; // handled below
            }
        });
    });
    ui.add_space(8.0);

    let mut action: Option<DlAction> = None;

    // Check limpiar completed
    // (handled through clear button above - need to do it differently)
    let mut clear_done = false;

    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        for (i, dl) in downloads.iter().enumerate() {
            Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(14.0)
                .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:8.0})
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let sico = match &dl.status { DownloadStatus::Queued=>"🕐", DownloadStatus::Downloading=>"⬇", DownloadStatus::Done=>"✅", DownloadStatus::Error(_)=>"❌" };
                        ui.label(egui::RichText::new(sico).size(22.0)); ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&dl.name).size(14.0).strong().color(name_col));
                            let url_short = &dl.url[..dl.url.len().min(60)];
                            ui.label(egui::RichText::new(format!("💾 {}  •  {}...", dl.display_size, url_short)).size(11.0).color(url_col).monospace());
                            if let DownloadStatus::Error(e) = &dl.status {
                                ui.label(egui::RichText::new(format!("Error: {}", e)).size(11.0).color(Color32::from_rgb(220,80,80)));
                            }
                            if dl.status == DownloadStatus::Downloading && !dl.speed_str.is_empty() {
                                let pct = (dl.progress * 100.0) as u32;
                                ui.label(egui::RichText::new(format!("{}% — {}", pct, dl.speed_str)).size(11.0).color(Color32::from_rgb(80,180,120)));
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new("✕").size(12.0)).fill(Color32::TRANSPARENT)).clicked() {
                                action = Some(DlAction::Remove(i));
                            }
                            if dl.status == DownloadStatus::Done {
                                ui.add_space(6.0);
                                if ui.add(egui::Button::new(egui::RichText::new("📁 Abrir").size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(30,80,40)).rounding(Rounding::same(6.0)).min_size(Vec2::new(80.0,28.0))).clicked() {
                                    action = Some(DlAction::OpenFile(i));
                                }
                            } else if dl.status == DownloadStatus::Queued {
                                ui.add_space(6.0);
                                if ui.add(egui::Button::new(egui::RichText::new("▶  Iniciar").size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(6.0)).min_size(Vec2::new(90.0,28.0))).clicked() {
                                    action = Some(DlAction::Start(i));
                                }
                            }
                        });
                    });
                    // Barra de progreso
                    if dl.status == DownloadStatus::Downloading {
                        ui.add_space(8.0);
                        let bw = ui.available_width() - 4.0;
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(bw, 8.0), egui::Sense::hover());
                        let pbg = match tema { Tema::Oscuro => Color32::from_rgb(25,25,38), Tema::Claro => Color32::from_rgb(220,222,240) };
                        ui.painter().rect_filled(rect, Rounding::same(4.0), pbg);
                        if dl.progress > 0.0 {
                            let fw = rect.width() * dl.progress;
                            let fr = egui::Rect::from_min_size(rect.min, Vec2::new(fw, rect.height()));
                            ui.painter().rect_filled(fr, Rounding::same(4.0), Color32::from_rgb(40,100,220));
                        }
                    }
                });
            let _ = clear_done;
        }
    });

    // Limpiar completadas via botón arriba
    if ui.add(egui::Button::new(egui::RichText::new("🗑  Limpiar completadas").size(12.0)).fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
        clear_done = true;
    }
    if clear_done { action = Some(DlAction::ClearDone); }

    action
}

// ─── ISOs Locales ─────────────────────────────────────────────────────────────

fn draw_locales(ui: &mut egui::Ui, iso_files: &[IsoFile], scan_dir: &str, tema: &Tema) -> bool {
    let card_bg  = match tema { Tema::Oscuro => Color32::from_rgb(22,22,32),    Tema::Claro => Color32::WHITE };
    let brd      = match tema { Tema::Oscuro => Color32::from_rgb(40,44,60),    Tema::Claro => Color32::from_rgb(210,215,230) };
    let name_col = match tema { Tema::Oscuro => Color32::WHITE,                 Tema::Claro => Color32::from_rgb(20,25,50) };
    let path_col = match tema { Tema::Oscuro => Color32::from_rgb(100,110,130), Tema::Claro => Color32::from_rgb(90,100,135) };
    let tc       = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };

    let mut rescan = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("📁  {}", scan_dir)).size(13.0).color(path_col).monospace());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(egui::RichText::new("🔄  Escanear").size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(7.0)).min_size(Vec2::new(100.0,28.0))).clicked() {
                rescan = true;
            }
        });
    });
    ui.add_space(10.0);

    if iso_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            let ic = match tema { Tema::Oscuro => Color32::from_rgb(60,65,90), Tema::Claro => Color32::from_rgb(150,160,195) };
            ui.label(egui::RichText::new("💿").size(44.0).color(ic)); ui.add_space(12.0);
            ui.label(egui::RichText::new("No se encontraron archivos ISO/IMG").size(15.0).color(tc)); ui.add_space(6.0);
            let t2 = match tema { Tema::Oscuro => Color32::from_rgb(90,95,115), Tema::Claro => Color32::from_rgb(110,120,150) };
            ui.label(egui::RichText::new("Descarga ISOs desde el Catálogo o ajusta el directorio en Configuración").size(12.0).color(t2));
        });
    } else {
        ui.label(egui::RichText::new(format!("{} archivo(s) encontrado(s)", iso_files.len())).size(13.0).color(tc));
        ui.add_space(8.0);
        egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
            for iso in iso_files {
                Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(14.0)
                    .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:8.0})
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("💿").size(24.0)); ui.add_space(8.0);
                            ui.vertical(|ui| {
                                ui.label(egui::RichText::new(&iso.name).size(14.0).strong().color(name_col));
                                ui.label(egui::RichText::new(&iso.path).size(11.0).color(path_col).monospace());
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(egui::RichText::new(format_size(iso.size_bytes)).size(13.0).color(tc));
                                ui.add_space(12.0);
                                if ui.add(egui::Button::new(egui::RichText::new("📁").size(14.0)).fill(Color32::TRANSPARENT)).clicked() {
                                    let _ = Command::new("xdg-open").arg(&iso.path.rsplit('/').skip(1).collect::<Vec<_>>().iter().rev().cloned().collect::<Vec<_>>().join("/")).spawn();
                                }
                            });
                        });
                    });
            }
        });
    }
    rescan
}

// ─── Configuración ────────────────────────────────────────────────────────────

fn draw_configuracion(ui: &mut egui::Ui, config: &mut AppConfig, tema: &Tema) {
    let sec_col  = match tema { Tema::Oscuro => Color32::from_rgb(80,140,255),  Tema::Claro => Color32::from_rgb(40,80,200) };
    let tc       = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };
    let card_bg  = match tema { Tema::Oscuro => Color32::from_rgb(22,22,32),    Tema::Claro => Color32::WHITE };
    let brd      = match tema { Tema::Oscuro => Color32::from_rgb(40,44,60),    Tema::Claro => Color32::from_rgb(210,215,230) };

    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        // ── Directorio de descargas ──
        Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(16.0)
            .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:14.0}).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(egui::RichText::new("📁  Directorio de descargas").size(14.0).strong().color(sec_col));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Las ISOs se guardan en esta carpeta.").size(12.0).color(tc));
                ui.add_space(8.0);
                ui.add(egui::TextEdit::singleline(&mut config.download_dir).desired_width(ui.available_width()).hint_text("/home/usuario/Descargas"));
            });

        // ── Velocidad de descarga ──
        Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(16.0)
            .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:14.0}).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(egui::RichText::new("⚡  Velocidad de descarga").size(14.0).strong().color(sec_col));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Limita la velocidad para no saturar la conexión durante el uso normal.").size(12.0).color(tc));
                ui.add_space(12.0);
                for variant in [SpeedLimit::Low, SpeedLimit::Medium, SpeedLimit::High, SpeedLimit::Max] {
                    let selected = config.speed_limit == variant;
                    let bg = if selected { Color32::from_rgb(40,80,180) } else {
                        match tema { Tema::Oscuro => Color32::from_rgb(30,30,45), Tema::Claro => Color32::from_rgb(220,222,238) }
                    };
                    let fg = if selected { Color32::WHITE } else { match tema { Tema::Oscuro => Color32::from_rgb(170,175,195), Tema::Claro => Color32::from_rgb(60,65,90) } };
                    if ui.add(egui::Button::new(egui::RichText::new(variant.label()).size(13.0).color(fg)).fill(bg).rounding(Rounding::same(7.0)).min_size(Vec2::new(260.0,32.0))).clicked() {
                        config.speed_limit = variant;
                    }
                    ui.add_space(4.0);
                }
            });

        // ── Logos ──
        Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(16.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new("🖼  Logos de distribuciones").size(14.0).strong().color(sec_col));
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Crea el directorio assets/logos/ en la raíz del proyecto y coloca archivos PNG de 64×64 px con los nombres listados abajo.").size(12.0).color(tc));
            ui.add_space(8.0);
            let logos = ["arch.png","manjaro.png","endeavouros.png","opensuse.png","ubuntu.png","mint.png","debian.png","fedora.png","antix.png","alpine.png","kali.png","tails.png","parrot.png","nobara.png","cachyos.png","bazzite.png","windows.png","almalinux.png"];
            ui.horizontal_wrapped(|ui| {
                for logo in logos {
                    let exists = std::path::Path::new("assets/logos").join(logo).exists();
                    let (bg, fg) = if exists { (Color32::from_rgb(20,70,30), Color32::from_rgb(80,220,100)) } else {
                        match tema { Tema::Oscuro => (Color32::from_rgb(30,30,45), Color32::from_rgb(130,140,160)), Tema::Claro => (Color32::from_rgb(230,232,245), Color32::from_rgb(100,110,140)) }
                    };
                    Frame::none().fill(bg).rounding(Rounding::same(5.0)).inner_margin(egui::Margin{left:7.0,right:7.0,top:3.0,bottom:3.0}).show(ui, |ui| {
                        ui.label(egui::RichText::new(logo).size(11.0).color(fg).monospace());
                    });
                    ui.add_space(3.0);
                }
            });
        });
    });
}

// ─── Logs ─────────────────────────────────────────────────────────────────────

fn draw_logs(ui: &mut egui::Ui, ctx: &egui::Context, op: &mut OpProgress, tema: &Tema) {
    let t = ctx.input(|i| i.time) as f32;
    if op.active {
        let dots = ".".repeat((t * 2.0) as usize % 4);
        ui.label(egui::RichText::new(format!("⚡ {}{}", op.label, dots)).size(15.0).strong().color(Color32::from_rgb(80,140,255)));
        ui.add_space(12.0);
        let pct = (op.progress * 100.0) as u32;
        let bw = ui.available_width() - 20.0;
        let (rect,_) = ui.allocate_exact_size(Vec2::new(bw, 28.0), egui::Sense::hover());
        let pbg = match tema { Tema::Oscuro => Color32::from_rgb(25,25,38), Tema::Claro => Color32::from_rgb(220,222,240) };
        let p = ui.painter();
        p.rect_filled(rect, Rounding::same(8.0), pbg);
        if op.progress > 0.0 {
            let fw = rect.width() * op.progress;
            let fr = egui::Rect::from_min_size(rect.min, Vec2::new(fw, rect.height()));
            p.rect_filled(fr, Rounding::same(8.0), Color32::from_rgb(30,80,200));
            let sh = egui::Rect::from_min_size(rect.min, Vec2::new(fw, rect.height()/2.0));
            p.rect_filled(sh, Rounding{nw:8.0,ne:8.0,sw:0.0,se:0.0}, Color32::from_rgba_premultiplied(80,140,255,60));
        }
        let bc = match tema { Tema::Oscuro => Color32::from_rgb(50,60,90), Tema::Claro => Color32::from_rgb(180,185,220) };
        p.rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0, bc));
        let pc = match tema { Tema::Oscuro => Color32::WHITE, Tema::Claro => Color32::from_rgb(20,30,70) };
        p.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}%", pct), egui::FontId::proportional(13.0), pc);
        ui.add_space(10.0);
        if let Some(last) = op.logs.last() {
            let lc = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(90,100,130) };
            ui.label(egui::RichText::new(format!("  {}", last.message)).size(12.0).color(lc).italics());
        }
        ui.add_space(16.0);
        if let Some(_) = &op.cancel_tx {
            if ui.add(egui::Button::new(egui::RichText::new("✕  Cancelar").size(13.0).color(Color32::from_rgb(220,80,80)))
                .fill(Color32::from_rgb(60,20,20)).rounding(Rounding::same(7.0)).min_size(Vec2::new(120.0,30.0))).clicked() {
                let tx = op.cancel_tx.take().unwrap();
                let _ = tx.send(()); op.active = false;
                op.add_log("Operación cancelada por el usuario", LogLevel::Warn);
            }
        }
    } else if !op.logs.is_empty() {
        let ok  = op.logs.iter().any(|l| l.level == LogLevel::Ok);
        let err = op.logs.iter().any(|l| l.level == LogLevel::Error);
        let (icon, txt, col) = if ok && !err { ("✅","Operación completada",Color32::from_rgb(80,200,120)) }
            else if err { ("❌","Operación con errores",Color32::from_rgb(220,80,80)) }
            else { ("⚠","Operación cancelada",Color32::from_rgb(220,180,60)) };
        ui.label(egui::RichText::new(format!("{icon}  {txt}")).size(15.0).strong().color(col));
        ui.add_space(12.0);
    } else {
        let ic = match tema { Tema::Oscuro => Color32::from_rgb(60,65,90), Tema::Claro => Color32::from_rgb(150,160,195) };
        let tc = match tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(80,90,120) };
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("📋").size(40.0).color(ic)); ui.add_space(10.0);
            ui.label(egui::RichText::new("Sin operaciones activas").size(14.0).color(tc));
        }); return;
    }
    if !op.logs.is_empty() {
        let tog = if op.logs_expanded { "▼  Ocultar logs" } else { "▶  Ver logs detallados" };
        if ui.add(egui::Button::new(egui::RichText::new(tog).size(13.0).color(Color32::from_rgb(100,140,220))).fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
            op.logs_expanded = !op.logs_expanded;
        }
        let anim = ctx.animate_bool_with_time(egui::Id::new("logs_expand"), op.logs_expanded, 0.20);
        if anim > 0.01 { ctx.request_repaint(); }
        if anim > 0.01 {
            ui.add_space(8.0);
            let lb = match tema { Tema::Oscuro => Color32::from_rgb(12,12,18), Tema::Claro => Color32::from_rgb(240,242,250) };
            let lbrd = match tema { Tema::Oscuro => Color32::from_rgb(40,44,65), Tema::Claro => Color32::from_rgb(200,205,225) };
            let lt = match tema { Tema::Oscuro => Color32::from_rgb(200,205,220), Tema::Claro => Color32::from_rgb(40,45,70) };
            Frame::none().fill(lb).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,lbrd)).inner_margin(12.0).show(ui, |ui| {
                ui.set_min_width(ui.available_width() - 20.0);
                egui::ScrollArea::vertical().max_height(200.0 * anim).stick_to_bottom(true).show(ui, |ui| {
                    for e in &op.logs {
                        let (pre, col) = match e.level {
                            LogLevel::Info  => ("INFO ", Color32::from_rgb(160,170,190)),
                            LogLevel::Ok    => ("OK   ", Color32::from_rgb(80,200,120)),
                            LogLevel::Warn  => ("WARN ", Color32::from_rgb(220,180,60)),
                            LogLevel::Error => ("ERR  ", Color32::from_rgb(220,80,80)),
                        };
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("[{}]", e.timestamp)).size(11.0).monospace().color(Color32::from_rgb(90,95,115)));
                            ui.label(egui::RichText::new(pre).size(11.0).monospace().color(col));
                            ui.label(egui::RichText::new(&e.message).size(11.0).monospace().color(lt));
                        });
                    }
                });
            });
        }
        ui.add_space(12.0);
        if !op.active {
            if ui.add(egui::Button::new(egui::RichText::new("🗑  Limpiar logs").size(12.0).color(Color32::from_rgb(180,80,80))).fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
                op.logs.clear(); op.logs_expanded = false;
            }
        }
    }
}

// ─── App ─────────────────────────────────────────────────────────────────────

struct IsoFlash {
    panel: Panel, tema: Tema, tema_anim: f32,
    usbs: Vec<UsbDevice>, scanning: bool, last_scan: f64,
    usb_rx: Option<Receiver<Vec<UsbDevice>>>,
    rescan_after: Option<f64>,   // tiempo en el que forzar rescan post-Ventoy
    op: OpProgress, op_rx: Option<Receiver<(f32, String, LogLevel, bool)>>,
    catalog: Vec<Distro>, cat_search: String, cat_filter: CatFilter, cat_win_popup: bool, cat_win_name: String,
    downloads: Vec<DownloadEntry>,
    iso_files: Vec<IsoFile>,
    config: AppConfig,
}

impl Default for IsoFlash {
    fn default() -> Self {
        Self {
            panel: Panel::Dashboard, tema: Tema::Oscuro, tema_anim: 0.0,
            usbs: vec![], scanning: false, last_scan: -999.0, usb_rx: None,
            rescan_after: None,
            op: OpProgress::default(), op_rx: None,
            catalog: build_catalog(), cat_search: String::new(), cat_filter: CatFilter::All, cat_win_popup: false, cat_win_name: String::new(),
            downloads: vec![], iso_files: vec![], config: AppConfig::default(),
        }
    }
}

impl IsoFlash {
    fn start_install_ventoy(&mut self, path: String, is_update: bool) {
        if self.op.active { return; }
        self.op = OpProgress::default();
        self.op.active = true;
        self.op.label = format!("{} Ventoy en {}", if is_update { "Actualizando" } else { "Instalando" }, path);
        self.op.add_log(&format!("Iniciando {} en {}", if is_update { "actualización" } else { "instalación" }, path), LogLevel::Info);

        let (tx, rx) = channel::<(f32, String, LogLevel, bool)>();
        let (ctx, crx) = channel::<()>();
        self.op.cancel_tx = Some(ctx);
        self.op_rx = Some(rx);

        std::thread::spawn(move || {
            let cancelled = || crx.try_recv().is_ok();
            let send = |p: f32, msg: &str, lvl: LogLevel, done: bool| { let _ = tx.send((p, msg.to_string(), lvl, done)); };

            // Verificar dispositivo
            send(0.05, &format!("Verificando dispositivo {}...", path), LogLevel::Info, false);
            match Command::new("lsblk").args([&path]).output() {
                Err(e) => { send(0.0, &format!("Error: {}", e), LogLevel::Error, true); return; }
                Ok(o) if !o.status.success() => { send(0.0, &format!("Dispositivo {} no encontrado", path), LogLevel::Error, true); return; }
                _ => {}
            }

            // Leer tamaño real
            send(0.10, "Leyendo información del dispositivo...", LogLevel::Info, false);
            if let Ok(o) = Command::new("lsblk").args(["-b","-n","-o","SIZE", &path]).output() {
                let txt = String::from_utf8_lossy(&o.stdout);
                if let Ok(bytes) = txt.lines().next().unwrap_or("").trim().parse::<u64>() {
                    send(0.15, &format!("Tamaño detectado: {:.1} GB", bytes as f64 / 1e9), LogLevel::Info, false);
                }
            }
            if cancelled() { send(0.0, "Cancelado", LogLevel::Warn, true); return; }

            // Buscar Ventoy2Disk.sh
            send(0.20, "Buscando Ventoy en el sistema...", LogLevel::Info, false);
            let bin_path = {
                let send_ref = &send;
                find_ventoy_bin(send_ref)
            };

            let bin = match bin_path {
                Some(b) => b,
                None => {
                    // Descargar desde GitHub Releases
                    send(0.28, "Ventoy no encontrado. Descargando ventoy 1.1.12...", LogLevel::Warn, false);
                    let url     = "https://github.com/ventoy/Ventoy/releases/download/v1.1.12/ventoy-1.1.12-linux.tar.gz";
                    let tmp_gz  = "/tmp/ventoy-isoflash.tar.gz";
                    let tmp_dir = "/tmp/ventoy-isoflash";
                    let _ = std::fs::remove_file(tmp_gz);
                    let _ = std::fs::remove_dir_all(tmp_dir);

                    let dl_ok = Command::new("wget").args(["-q","-O", tmp_gz, url]).status().map(|s| s.success()).unwrap_or(false)
                        || Command::new("curl").args(["-L","-o", tmp_gz, url]).status().map(|s| s.success()).unwrap_or(false);

                    if !dl_ok { send(0.0, "Descarga fallida. Instala manualmente: paru -S ventoy", LogLevel::Error, true); return; }
                    if cancelled() { send(0.0, "Cancelado", LogLevel::Warn, true); return; }

                    send(0.45, "Extrayendo paquete Ventoy...", LogLevel::Info, false);
                    let _ = std::fs::create_dir_all(tmp_dir);
                    if let Ok(o) = Command::new("tar").args(["-xzf", tmp_gz, "-C", tmp_dir]).output() {
                        if !o.status.success() { send(0.0, "Error extrayendo el paquete", LogLevel::Error, true); return; }
                    }

                    // Buscar el script en el dir extraído
                    let found = Command::new("find").args([tmp_dir, "-name", "Ventoy2Disk.sh", "-maxdepth", "3"])
                        .output().ok().and_then(|o| {
                            String::from_utf8_lossy(&o.stdout).lines().next().map(|l| l.trim().to_string())
                        }).filter(|s| !s.is_empty());

                    match found {
                        Some(p) => { send(0.48, &format!("Script encontrado: {}", p), LogLevel::Info, false); p }
                        None => { send(0.0, "No se encontró Ventoy2Disk.sh en el paquete descargado", LogLevel::Error, true); return; }
                    }
                }
            };

            if cancelled() { send(0.0, "Cancelado", LogLevel::Warn, true); return; }
            send(0.50, &format!("Usando: {}", bin), LogLevel::Info, false);
            send(0.55, "Ejecutando instalación — se pedirá contraseña de administrador...", LogLevel::Warn, false);

            // Hacer ejecutable si es script
            if bin.ends_with(".sh") { let _ = Command::new("chmod").args(["+x",&bin]).output(); }

            // Elegir flag según operación
            let flag = if is_update { "-u" } else { "-I" };
            let result = Command::new("pkexec").args(["bash", &bin, flag, &path]).output();

            match result {
                Err(e) => { send(1.0, &format!("Error ejecutando pkexec: {}", e), LogLevel::Error, true); }
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();
                    if o.status.success() || stdout.to_lowercase().contains("done") || stdout.to_lowercase().contains("ventoy") {
                        send(0.90, "Particionando y copiando archivos Ventoy...", LogLevel::Info, false);
                        for line in stdout.lines().take(5) {
                            let l = line.trim();
                            if !l.is_empty() && !l.starts_with('*') { send(0.95, l, LogLevel::Info, false); }
                        }
                        // Esperar que el SO actualice la tabla de particiones
                        std::thread::sleep(Duration::from_secs(2));
                        send(1.0, "¡Ventoy instalado correctamente! Actualizando lista...", LogLevel::Ok, true);
                    } else {
                        if o.status.code() == Some(127) {
                            send(1.0, "Cancelado: no se proporcionó contraseña de administrador", LogLevel::Warn, true);
                        } else {
                            for line in stderr.lines().take(4) {
                                let l = line.trim();
                                if !l.is_empty() { send(0.0, l, LogLevel::Error, false); }
                            }
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
                let mut v = Visuals::dark();
                v.panel_fill = Color32::from_rgb(15,15,20); v.window_fill = Color32::from_rgb(20,20,28);
                v.extreme_bg_color = Color32::from_rgb(10,10,14); v.faint_bg_color = Color32::from_rgb(25,25,35);
                v.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(200,205,220);
                v.widgets.inactive.bg_fill = Color32::from_rgb(30,30,42); v.widgets.inactive.rounding = Rounding::same(8.0);
                v.widgets.inactive.fg_stroke.color = Color32::from_rgb(180,185,200);
                v.widgets.hovered.bg_fill = Color32::from_rgb(50,100,200); v.widgets.hovered.rounding = Rounding::same(8.0);
                v.widgets.active.bg_fill = Color32::from_rgb(40,80,180); v.widgets.active.rounding = Rounding::same(8.0);
                v.selection.bg_fill = Color32::from_rgb(40,80,180); v.override_text_color = None;
                ctx.set_visuals(v);
            }
            Tema::Claro => {
                let mut v = Visuals::light();
                v.panel_fill = Color32::from_rgb(245,246,250); v.window_fill = Color32::WHITE;
                v.extreme_bg_color = Color32::from_rgb(230,232,240);
                v.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(50,55,80);
                v.widgets.noninteractive.bg_fill = Color32::from_rgb(245,246,250);
                v.widgets.inactive.bg_fill = Color32::from_rgb(225,227,240); v.widgets.inactive.rounding = Rounding::same(8.0);
                v.widgets.inactive.fg_stroke.color = Color32::from_rgb(55,60,90);
                v.widgets.hovered.bg_fill = Color32::from_rgb(100,140,230); v.widgets.hovered.rounding = Rounding::same(8.0);
                v.widgets.hovered.fg_stroke.color = Color32::WHITE;
                v.widgets.active.bg_fill = Color32::from_rgb(70,110,210); v.widgets.active.rounding = Rounding::same(8.0);
                v.widgets.active.fg_stroke.color = Color32::WHITE;
                v.selection.bg_fill = Color32::from_rgb(70,110,210);
                v.override_text_color = Some(Color32::from_rgb(25,30,55));
                ctx.set_visuals(v);
            }
        }
    }
}

impl eframe::App for IsoFlash {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);

        // ── Auto-scan silencioso cada 2s ──
        if !self.scanning && (now - self.last_scan) >= 2.0 {
            self.last_scan = now;
            self.scanning = true;
            let (tx, rx) = channel();
            self.usb_rx = Some(rx);
            std::thread::spawn(move || { let _ = tx.send(scan_usbs()); });
        }
        if let Some(rx) = &self.usb_rx {
            if let Ok(usbs) = rx.try_recv() {
                self.usbs = usbs; self.scanning = false; self.usb_rx = None;
            }
        }

        // ── Rescan forzado post-Ventoy ──
        if let Some(at) = self.rescan_after {
            if now >= at { self.rescan_after = None; self.last_scan = -999.0; }
        }

        // ── Progreso operación Ventoy ──
        let mut ventoy_ok = false;
        if let Some(rx) = &self.op_rx {
            while let Ok((progress, msg, level, done)) = rx.try_recv() {
                if progress > 0.0 { self.op.progress = progress; }
                if done && level == LogLevel::Ok { ventoy_ok = true; }
                self.op.add_log(&msg, level);
                if done { self.op.active = false; self.op.cancel_tx = None; self.op_rx = None; break; }
            }
        }
        if ventoy_ok { self.rescan_after = Some(now + 1.5); }

        // ── Progreso descargas ──
        for dl in &mut self.downloads {
            if let Some(rx) = &dl.progress_rx {
                while let Ok(p) = rx.try_recv() {
                    if let Some(e) = &p.error {
                        dl.status = DownloadStatus::Error(e.clone()); dl.progress_rx = None; break;
                    } else if p.done {
                        dl.status = DownloadStatus::Done; dl.progress = 1.0; dl.progress_rx = None; break;
                    } else {
                        dl.progress = p.progress; dl.speed_str = p.speed;
                    }
                }
            }
        }

        // ── Animación tema suave ──
        let target = match self.tema { Tema::Oscuro => 0.0_f32, Tema::Claro => 1.0_f32 };
        self.tema_anim += (target - self.tema_anim) * 0.10;
        if (self.tema_anim - target).abs() > 0.002 { ctx.request_repaint(); }
        let pd = Color32::from_rgb(15,15,20); let pl = Color32::from_rgb(245,246,250);
        let sd = Color32::from_rgb(18,18,26); let sl = Color32::from_rgb(235,237,245);
        let panel_now   = lerp_color(pd, pl, self.tema_anim);
        let sidebar_now = lerp_color(sd, sl, self.tema_anim);
        let text_now    = lerp_color(Color32::from_rgb(200,205,220), Color32::from_rgb(25,30,55), self.tema_anim);
        let stroke_now  = lerp_color(Color32::from_rgb(200,205,220), Color32::from_rgb(50,55,80), self.tema_anim);

        { 
            let mut v = ctx.style().visuals.clone(); 
            v.panel_fill = panel_now; 
            v.override_text_color = Some(text_now);
            v.widgets.noninteractive.fg_stroke.color = stroke_now;
            v.widgets.inactive.fg_stroke.color = stroke_now;
            ctx.set_visuals(v); 
        }

        if self.op.active { ctx.request_repaint(); }
        let t = ctx.input(|i| i.time) as f32;
        let pulse = ((t * 1.8).sin() * 0.12 + 0.88).clamp(0.0,1.0);
        let logo_col = Color32::from_rgb((80.0*pulse) as u8, (140.0*pulse) as u8, (255.0*pulse) as u8);
        ctx.request_repaint_after(Duration::from_millis(50));

        let op_active = self.op.active;
        let op_cancel = self.op.cancel_tx.is_some();
        let has_downloads = !self.downloads.is_empty();

        // ── Sidebar ──
        egui::SidePanel::left("sidebar").exact_width(170.0)
            .frame(Frame::none().fill(sidebar_now).inner_margin(10.0))
            .show(ctx, |ui| {
                ui.add_space(16.0);
                ui.label(egui::RichText::new("⚡ IsoFlash").size(20.0).strong().color(logo_col));
                ui.add_space(24.0);
                sidebar_btn(ui,ctx,&mut self.panel,&self.tema,Panel::Dashboard,    "🖥","Dashboard",    false); ui.add_space(4.0);
                sidebar_btn(ui,ctx,&mut self.panel,&self.tema,Panel::Catalogo,     "📦","Catálogo",     false); ui.add_space(4.0);
                sidebar_btn(ui,ctx,&mut self.panel,&self.tema,Panel::Descargas,    "⬇","Descargas",    has_downloads); ui.add_space(4.0);
                sidebar_btn(ui,ctx,&mut self.panel,&self.tema,Panel::Locales,      "💾","ISOs Locales", false); ui.add_space(4.0);
                sidebar_btn(ui,ctx,&mut self.panel,&self.tema,Panel::Flasheo,      "🔥","Flasheo",      false); ui.add_space(4.0);
                sidebar_btn(ui,ctx,&mut self.panel,&self.tema,Panel::Persistencia, "💿","Persistencia", false); ui.add_space(4.0);
                sidebar_btn(ui,ctx,&mut self.panel,&self.tema,Panel::Logs,         "📋","Logs",         op_active);
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(8.0);
                    let (ico, lbl) = match self.tema { Tema::Oscuro => ("☀","Tema Claro"), Tema::Claro => ("🌙","Tema Oscuro") };
                    let tfc = match self.tema { Tema::Oscuro => Color32::from_rgb(180,185,200), Tema::Claro => Color32::from_rgb(60,65,90) };
                    if ui.add(egui::Button::new(egui::RichText::new(format!("{ico}  {lbl}")).size(13.0).color(tfc)).fill(sidebar_now).rounding(Rounding::same(8.0)).min_size(Vec2::new(150.0,34.0))).clicked() {
                        self.tema = match self.tema { Tema::Oscuro => Tema::Claro, Tema::Claro => Tema::Oscuro };
                        self.apply_theme(ctx);
                    }
                    ui.add_space(4.0);
                    sidebar_btn(ui,ctx,&mut self.panel,&self.tema,Panel::Configuracion,"⚙","Configuración",false);
                });
            });

        let mut dash_action: Option<DashAction> = None;

        egui::CentralPanel::default()
            .frame(Frame::none().fill(panel_now).inner_margin(egui::Margin{left:20.0,right:20.0,top:0.0,bottom:0.0}))
            .show(ctx, |ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    let (tit, sub) = match self.panel {
                        Panel::Dashboard     => ("Dashboard",     "USBs conectados y estado Ventoy"),
                        Panel::Catalogo      => ("Catálogo",      "Descarga ISOs verificadas"),
                        Panel::Descargas     => ("Descargas",     "Cola de descargas activa"),
                        Panel::Locales       => ("ISOs Locales",  "Archivos ISO en tu sistema"),
                        Panel::Flasheo       => ("Flasheo",       "Escribe ISOs a tus USBs"),
                        Panel::Persistencia  => ("Persistencia",  "Configura almacenamiento persistente"),
                        Panel::Logs          => ("Logs",          "Progreso y detalles de operaciones"),
                        Panel::Configuracion => ("Configuración", "Ajustes de la aplicación"),
                    };
                    let fade = ctx.animate_value_with_time(egui::Id::new("panel_fade"), 1.0, 0.25);
                    let a = (fade * 255.0) as u8;
                    let sc = match self.tema { Tema::Oscuro => Color32::from_rgba_premultiplied(130,140,160,a), Tema::Claro => Color32::from_rgba_premultiplied(75,85,120,a) };
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(tit).size(26.0).strong().color(Color32::from_rgba_premultiplied(60,120,240,a)));
                        ui.label(egui::RichText::new(sub).size(13.0).color(sc));
                    });
                });
                ui.add_space(10.0); ui.separator(); ui.add_space(12.0);

                match self.panel {
                    Panel::Dashboard => draw_dashboard(ui, ctx, &self.usbs, self.scanning, op_active, op_cancel, &self.tema, &mut dash_action),
                    Panel::Catalogo  => draw_catalog(ui, &self.catalog, &mut self.cat_search, &mut self.cat_filter, &mut self.cat_win_popup, &mut self.cat_win_name, &mut self.downloads, &self.config, &self.tema),
                    Panel::Descargas => {
                        if let Some(act) = draw_descargas(ui, ctx, &mut self.downloads, &self.config, &self.tema) {
                            match act {
                                DlAction::Start(i) => { start_download(&mut self.downloads[i], &self.config); }
                                DlAction::Remove(i) => { self.downloads.remove(i); }
                                DlAction::OpenFile(i) => {
                                    if let Some(parent) = std::path::Path::new(&self.downloads[i].dest_path).parent() {
                                        let _ = Command::new("xdg-open").arg(parent).spawn();
                                    }
                                }
                                DlAction::ClearDone => { self.downloads.retain(|d| d.status != DownloadStatus::Done); }
                            }
                        }
                    }
                    Panel::Locales => {
                        if draw_locales(ui, &self.iso_files, &self.config.download_dir, &self.tema) {
                            self.iso_files = scan_iso_files(&self.config.download_dir);
                        }
                    }
                    Panel::Configuracion => draw_configuracion(ui, &mut self.config, &self.tema),
                    Panel::Logs => draw_logs(ui, ctx, &mut self.op, &self.tema),
                    _ => {
                        let c = match self.tema { Tema::Oscuro => Color32::from_rgb(130,140,160), Tema::Claro => Color32::from_rgb(100,110,140) };
                        ui.vertical_centered(|ui| { ui.add_space(80.0); ui.label(egui::RichText::new("🚧  En construcción").size(16.0).color(c)); });
                    }
                }
            });

        // ── Popup Windows ──
        if self.cat_win_popup {
            let url = if self.cat_win_name.contains("11") { "https://www.microsoft.com/software-download/windows11" } else { "https://www.microsoft.com/software-download/windows10" };
            egui::Window::new(format!("🪟  {} — Descarga especial", self.cat_win_name)).collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0,0.0]).fixed_size([440.0,0.0])
                .show(ctx, |ui| {
                    ui.add_space(6.0);
                    Frame::none().fill(Color32::from_rgb(60,40,10)).rounding(Rounding::same(8.0)).inner_margin(12.0).show(ui, |ui| {
                        ui.label(egui::RichText::new("⚠  Windows no permite descarga directa de ISOs").size(13.0).strong().color(Color32::from_rgb(230,170,60)));
                    });
                    ui.add_space(10.0);
                    ui.label("Microsoft exige aceptar términos de licencia. IsoFlash no puede automatizar ese proceso.");
                    ui.add_space(8.0);
                    let pasos_col = match self.tema { Tema::Oscuro => Color32::WHITE, Tema::Claro => Color32::from_rgb(20,25,50) };
                    ui.label(egui::RichText::new("Pasos:").size(13.0).strong().color(pasos_col));
                    ui.label("1.  Visita el enlace oficial"); ui.label("2.  Elige idioma y edición"); ui.label("3.  Descarga la ISO"); ui.label("4.  Agrégala en ISOs Locales");
                    ui.add_space(10.0);
                    Frame::none().fill(match self.tema { Tema::Oscuro => Color32::from_rgb(20,20,30), Tema::Claro => Color32::from_rgb(235,238,250) }).rounding(Rounding::same(6.0)).inner_margin(8.0).show(ui, |ui| {
                        ui.label(egui::RichText::new(url).size(11.0).monospace().color(Color32::from_rgb(80,160,240)));
                    });
                    ui.add_space(12.0);
                    if ui.add(egui::Button::new(egui::RichText::new("Cerrar").size(13.0).color(Color32::WHITE)).fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(7.0)).min_size(Vec2::new(100.0,30.0))).clicked() {
                        self.cat_win_popup = false;
                    }
                    ui.add_space(4.0);
                });
        }

        // ── Acciones Dashboard ──
        if let Some(action) = dash_action {
            match action {
                DashAction::InstallVentoy(path, is_update) => { self.start_install_ventoy(path, is_update); }
                DashAction::CancelVentoy => {
                    if let Some(tx) = self.op.cancel_tx.take() { let _ = tx.send(()); }
                    self.op.active = false;
                    self.op.add_log("Operación cancelada por el usuario", LogLevel::Warn);
                }
                DashAction::GoFlash(_path) => self.panel = Panel::Flasheo,
            }
        }
    }
}
