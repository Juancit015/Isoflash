#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use egui::{Color32, Frame, Rounding, Stroke, Vec2, Visuals};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

mod resolver;

// ─── Constantes ───────────────────────────────────────────────────────────────

const CATALOG_JSON: &str = include_str!("../catalog.json");
const CATALOG_URL:  &str = "https://raw.githubusercontent.com/Juancit015/Isoflash/main/catalog.json";
const VENTOY_LOCAL: &str = "src/ventoy-1.1.12";

// ─── i18n / Lenguaje ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum Language { English, Spanish, Chinese }

impl Default for Language {
    fn default() -> Self {
        // Detectar del entorno Linux: LANG, LC_ALL, LC_MESSAGES
        let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default()
        .to_lowercase();
        if lang.starts_with("zh") || lang.contains("zh_cn") {
            Language::Chinese
        } else if lang.starts_with("es") {
            Language::Spanish
        } else {
            Language::English
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::English=>write!(f,"English"), Self::Spanish=>write!(f,"Spanish"), Self::Chinese=>write!(f,"Chinese") }
    }
}

fn load_i18n(lang: Language) -> HashMap<String, String> {
    let json = match lang {
        Language::English => include_str!("i18n/en.json"),
        Language::Spanish => include_str!("i18n/es.json"),
        Language::Chinese => include_str!("i18n/zh.json"),
    };
    serde_json::from_str(json).unwrap_or_default()
}

// Persistencia de config (idioma, directorio, velocidad)
fn save_app_config(lang: Language, download_dir: &str, speed_limit: &SpeedLimit) {
    let dir = format!("{}/.config/isoflash", std::env::var("HOME").unwrap_or_else(|_| ".".into()));
    let _ = std::fs::create_dir_all(&dir);
    let j = serde_json::json!({ "lang": lang.to_string(), "download_dir": download_dir, "speed_limit": speed_limit.label_key() });
    let _ = std::fs::write(format!("{}/config.json", dir), serde_json::to_string_pretty(&j).unwrap_or_default());
}

fn load_app_config() -> Option<(Language, String, String)> {
    let path = format!("{}/config.json", config_dir());
    let s = std::fs::read_to_string(&path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    let lang = match v["lang"].as_str().unwrap_or("English") {
        "Spanish" => Language::Spanish,
        "Chinese" => Language::Chinese,
        _ => Language::English,
    };
    let d = v["download_dir"].as_str().unwrap_or("").to_string();
    let sp = v["speed_limit"].as_str().unwrap_or("Max").to_string();
    Some((lang, d, sp))
}

fn load_app_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("logo/isoflashLogo.png");
    let img   = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData { rgba: img.into_raw(), width: w, height: h })
}

fn main() -> eframe::Result<()> {
    let icon = load_app_icon();
    let mut vp = egui::ViewportBuilder::default()
    .with_title("IsoFlash")
    .with_inner_size([980.0, 640.0])
    .with_min_inner_size([720.0, 420.0]);
    if let Some(ic) = icon { vp = vp.with_icon(ic); }
    let options = eframe::NativeOptions { viewport: vp, ..Default::default() };
    eframe::run_native("IsoFlash", options, Box::new(|cc| {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let app = IsoFlash::default();
        app.apply_theme(&cc.egui_ctx);
        Box::new(app)
    }))
}

// ─── Enums ────────────────────────────────────────────────────────────────────

#[derive(Default, PartialEq, Clone, Debug)]
enum Panel { #[default] Dashboard, Catalogo, Descargas, Locales, Flasheo, Persistencia, Logs, Configuracion }

#[derive(Default, PartialEq, Clone)]
enum Tema { #[default] Oscuro, Claro }

struct ThemeColors {
    card_bg: Color32, border: Color32, text: Color32, text_muted: Color32,
    text_dim: Color32, text_title: Color32, text_path: Color32,
    icon_empty: Color32,
    section_title: Color32, bar_bg: Color32,
    badge_bg: Color32, badge_fg: Color32,
    label_dim: Color32,
    btn_clear_fill: Color32, btn_clear_fg: Color32,
    sug_bg: Color32,
    sidebar_fill: Color32, panel_fill: Color32,
    dash_no_ventoy_bg: Color32, dash_no_ventoy_fg: Color32,
}

impl Tema {
    fn colors(&self) -> ThemeColors {
        match self {
            Tema::Oscuro => ThemeColors {
                card_bg: Color32::from_rgb(22,22,32), border: Color32::from_rgb(40,44,60),
                text: Color32::from_rgb(130,140,160), text_muted: Color32::from_rgb(100,110,130),
                text_dim: Color32::from_rgb(90,95,115), text_title: Color32::WHITE,
                text_path: Color32::from_rgb(130,140,160),
                icon_empty: Color32::from_rgb(60,65,90),
                section_title: Color32::from_rgb(80,140,255), bar_bg: Color32::from_rgb(18,18,28),
                badge_bg: Color32::from_rgb(30,35,55), badge_fg: Color32::from_rgb(180,190,220),
                label_dim: Color32::from_rgb(160,170,190),
                btn_clear_fill: Color32::from_rgb(30,30,45), btn_clear_fg: Color32::from_rgb(160,170,190),
                sug_bg: Color32::from_rgb(28,28,40),
                sidebar_fill: Color32::from_rgb(18,18,26), panel_fill: Color32::from_rgb(15,15,20),
                dash_no_ventoy_bg: Color32::from_rgb(50,50,70), dash_no_ventoy_fg: Color32::from_rgb(130,140,160),
            },
            Tema::Claro => ThemeColors {
                card_bg: Color32::WHITE, border: Color32::from_rgb(210,215,230),
                text: Color32::from_rgb(80,90,120), text_muted: Color32::from_rgb(100,110,140),
                text_dim: Color32::from_rgb(110,120,150), text_title: Color32::from_rgb(20,25,50),
                text_path: Color32::from_rgb(90,100,135),
                icon_empty: Color32::from_rgb(150,160,195),
                section_title: Color32::from_rgb(40,80,200), bar_bg: Color32::from_rgb(230,232,245),
                badge_bg: Color32::from_rgb(220,225,245), badge_fg: Color32::from_rgb(60,70,120),
                label_dim: Color32::from_rgb(60,65,90),
                btn_clear_fill: Color32::from_rgb(225,227,240), btn_clear_fg: Color32::from_rgb(70,75,100),
                sug_bg: Color32::from_rgb(240,242,255),
                sidebar_fill: Color32::from_rgb(235,237,245), panel_fill: Color32::from_rgb(245,246,250),
                dash_no_ventoy_bg: Color32::from_rgb(220,220,235), dash_no_ventoy_fg: Color32::from_rgb(100,105,140),
            },
        }
    }
}

#[derive(Default, PartialEq, Clone, Debug)]
enum CatFilter { #[default] All, Rolling, Lts, NoSystemd, Server, Security, Gaming, Windows }

#[derive(Clone, Debug, PartialEq)]
enum DownloadStatus { Queued, Downloading, Paused, Done, Error(String) }

#[derive(Clone, Default, PartialEq)]
enum SpeedLimit { Low, Medium, High, #[default] Max }

impl SpeedLimit {
    fn rate_arg(&self) -> Option<&'static str> {
        match self { Self::Low=>"500k", Self::Medium=>"2m", Self::High=>"8m", Self::Max=>return None }
        .into()
    }
    fn label_key(&self) -> &'static str {
        match self { Self::Low=>"cfg_speed_low", Self::Medium=>"cfg_speed_medium", Self::High=>"cfg_speed_high", Self::Max=>"cfg_speed_max" }
    }
    fn label(&self, i18n: &HashMap<String,String>) -> String {
        i18n.get(self.label_key()).cloned().unwrap_or_else(|| "?".into())
    }
}

fn hash_str(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

// ─── Structs ──────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct UsbDevice { model: String, size_bytes: u64, path: String, has_ventoy: bool }

#[derive(Clone)]
struct Distro {
    name: String, logo: String, description: String,
    category: CatFilter, arch: String, size_mb: u64, url: String, is_windows: bool,
}

struct DlProgress { progress: f32, speed: String, done: bool, error: Option<String> }

struct DownloadEntry {
    name: String, url: String, display_size: String, dest_path: String,
    status: DownloadStatus, progress: f32, speed_str: String,
    progress_rx: Option<Receiver<DlProgress>>,
    pause_tx:    Option<Sender<()>>,
}

#[derive(Clone)]
struct IsoFile { name: String, path: String, size_bytes: u64 }

#[derive(Clone)]
struct AppConfig { download_dir: String, speed_limit: SpeedLimit }

impl Default for AppConfig {
    fn default() -> Self { Self { download_dir: default_download_dir(), speed_limit: SpeedLimit::Max } }
}

#[derive(Clone, Debug)]
struct LogEntry { timestamp: String, message: String, level: LogLevel }

#[derive(Clone, Debug, PartialEq)]
enum LogLevel { Info, Ok, Warn, Error }

#[derive(Default)]
struct OpProgress {
    label: String, progress: f32, active: bool,
    logs: Vec<LogEntry>, logs_expanded: bool,
    cancel_tx: Option<Sender<()>>,
}

impl OpProgress {
    fn log(&mut self, msg: &str, level: LogLevel) {
        let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_secs();
        self.logs.push(LogEntry {
            timestamp: format!("{:02}:{:02}:{:02}", (s/3600)%24, (s/60)%60, s%60),
                       message: msg.to_string(), level,
        });
    }
}

enum DashAction { InstallVentoy(String, bool), CancelVentoy, GoFlash(String) }
enum DlAction { Start(usize), Pause(usize), Remove(usize), OpenDir(usize), ClearDone }

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn format_size(b: u64) -> String {
    if b >= 1_000_000_000 { format!("{:.1} GB", b as f64/1e9) }
    else if b >= 1_000_000 { format!("{:.0} MB", b as f64/1e6) }
    else if b >= 1_000     { format!("{:.0} KB", b as f64/1e3) }
    else                   { format!("{} B", b) }
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0,1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32)*t) as u8,
                      (a.g() as f32 + (b.g() as f32 - a.g() as f32)*t) as u8,
                      (a.b() as f32 + (b.b() as f32 - a.b() as f32)*t) as u8,
    )
}

fn safe_name(s: &str) -> String {
    s.chars().map(|c| if c.is_alphanumeric()||c=='-'||c=='_' {c} else {'_'}).collect()
}

fn default_download_dir() -> String {
    let h = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let d = format!("{}/Descargas", h);
    if std::path::Path::new(&d).exists() { d } else { format!("{}/Downloads", h) }
}

fn config_dir() -> String {
    format!("{}/.config/isoflash", std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

fn logo_uri(file: &str) -> Option<String> {
    // Buscar relativo al binario
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("icons").join(file);
            if p.exists() { return Some(format!("file://{}", p.display())); }
        }
    }
    // Fallback: relativo a CWD
    let p = std::path::Path::new("icons").join(file);
    if p.exists() { Some(format!("file://{}", p.display())) } else { None }
}

// Sugerencias de rutas para autocompletado
fn path_suggestions(partial: &str) -> Vec<String> {
    if partial.is_empty() { return vec![]; }
    let p = std::path::Path::new(partial);
    let (dir, prefix): (String, String) = if partial.ends_with('/') {
        (partial.to_string(), String::new())
    } else {
        let parent = p.parent().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(||"/".into());
        let name   = p.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        (parent, name)
    };
    std::fs::read_dir(&dir).ok().into_iter().flatten().flatten()
    .filter(|e| e.path().is_dir())
    .map(|e| {
        let d = dir.trim_end_matches('/');
        format!("{}/{}", d, e.file_name().to_string_lossy())
    })
    .filter(|s| {
        let last = s.rsplit('/').next().unwrap_or("").to_lowercase();
        last.starts_with(&prefix.to_lowercase())
    })
    .take(6)
    .collect()
}

// Validar ruta de descarga
fn validate_download_dir(path: &str, i18n: &HashMap<String,String>) -> Result<(), String> {
    let t = |k:&str| i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    if path.is_empty() { return Err(t("cfg_path_empty")); }
    let p = std::path::Path::new(path);
    if !p.exists() { return Err(t("cfg_path_not_exist")); }
    if !p.is_dir() { return Err(t("cfg_path_not_dir")); }
    Ok(())
}

// ─── Catalogo ─────────────────────────────────────────────────────────────────

fn parse_category(s: &str) -> CatFilter {
    match s {
        "rolling"   => CatFilter::Rolling,
        "lts"       => CatFilter::Lts,
        "nosystemd" => CatFilter::NoSystemd,
        "server"    => CatFilter::Server,
        "security"  => CatFilter::Security,
        "gaming"    => CatFilter::Gaming,
        "windows"   => CatFilter::Windows,
        _           => CatFilter::All,
    }
}

fn load_catalog(json: &str) -> Vec<Distro> {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v, Err(_) => return vec![],
    };
    v["distros"].as_array().unwrap_or(&vec![]).iter().filter_map(|d| {
        Some(Distro {
            name:        d["name"].as_str()?.to_string(),
             logo:        d["logo"].as_str().unwrap_or("").to_string(),
             description: d["description"].as_str().unwrap_or("").to_string(),
             category:    parse_category(d["category"].as_str().unwrap_or("lts")),
             arch:        d["arch"].as_str().unwrap_or("x86_64").to_string(),
             size_mb:     d["size_mb"].as_u64().unwrap_or(0),
             url:         d["url"].as_str().unwrap_or("").to_string(),
             is_windows:  d["is_windows"].as_bool().unwrap_or(false),
        })
    }).collect()
}

fn cat_badge(cat: &CatFilter, th: &ThemeColors, i18n: &HashMap<String,String>) -> (Color32, Color32, String) {
    let k = match cat {
        CatFilter::All => "cat_badge_all", CatFilter::Rolling => "cat_badge_rolling",
        CatFilter::Lts => "cat_badge_lts", CatFilter::NoSystemd => "cat_badge_nosystemd",
        CatFilter::Server => "cat_badge_server", CatFilter::Security => "cat_badge_security",
        CatFilter::Gaming => "cat_badge_gaming", CatFilter::Windows => "cat_badge_windows",
    };
    let txt = i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    (th.badge_bg, th.badge_fg, txt)
}

// ─── USB Scan ─────────────────────────────────────────────────────────────────

fn scan_usbs() -> Vec<UsbDevice> {
    // Intento 1: lsblk JSON (disponible en la mayoria de distros)
    if let Some(usbs) = scan_usbs_lsblk() { return usbs; }
    // Intento 2: /sys/block/ (Alpine, sistemas minimos)
    scan_usbs_sysfs()
}

fn scan_usbs_lsblk() -> Option<Vec<UsbDevice>> {
    let out = Command::new("lsblk").args(["-J","-b","-o","NAME,SIZE,MODEL,TRAN,TYPE"]).output().ok()?;
    if !out.status.success() { return None; }
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).ok()?;
    Some(v["blockdevices"].as_array()?.iter().filter_map(|dev| {
        if dev["tran"].as_str().unwrap_or("")!="usb" || dev["type"].as_str().unwrap_or("")!="disk" { return None; }
        let name        = dev["name"].as_str().unwrap_or("").to_string();
        let model       = dev["model"].as_str().unwrap_or("USB Device").trim().to_string();
        let size_bytes  = dev["size"].as_u64()
        .or_else(||dev["size"].as_str().and_then(|s|s.parse().ok())).unwrap_or(0);
        let path        = format!("/dev/{}", name);
        let has_ventoy  = check_ventoy(&name);
        Some(UsbDevice { model, size_bytes, path, has_ventoy })
    }).collect())
}

fn scan_usbs_sysfs() -> Vec<UsbDevice> {
    let mut result = vec![];
    let Ok(entries) = std::fs::read_dir("/sys/block") else { return result };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("sd") && !name.starts_with("vd") { continue; }
        // Verificar si es removible
        let removable = std::fs::read_to_string(format!("/sys/block/{}/removable",&name))
        .map(|s| s.trim()=="1").unwrap_or(false);
        if !removable { continue; }
        // Verificar que sea USB via uevent
        let uevent = std::fs::read_to_string(format!("/sys/block/{}/device/uevent",&name))
        .unwrap_or_default().to_lowercase();
        if !uevent.contains("usb") && !uevent.is_empty() { continue; } // si hay uevent y no es usb, saltar
        let size_bytes = std::fs::read_to_string(format!("/sys/block/{}/size",&name)).ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|blocks| blocks * 512).unwrap_or(0);
        let model = std::fs::read_to_string(format!("/sys/block/{}/device/model",&name))
        .unwrap_or_else(|_|"USB Device".into()).trim().to_string();
        let path = format!("/dev/{}", name);
        let has_ventoy = check_ventoy(&name);
        result.push(UsbDevice { model, size_bytes, path, has_ventoy });
    }
    result
}

// Verifica si un dispositivo tiene Ventoy instalado usando -l para ver todas las particiones
fn check_ventoy(dev_name: &str) -> bool {
    let path = format!("/dev/{}", dev_name);
    // lsblk -l lista el disco y sus particiones en modo plano
    if let Ok(o) = Command::new("lsblk").args(["-l","-o","NAME,LABEL,PARTLABEL",&path]).output() {
        if String::from_utf8_lossy(&o.stdout).to_lowercase().contains("ventoy") { return true; }
    }
    // Fallback: revisar /sys/class/block para cada particion
    if let Ok(entries) = std::fs::read_dir("/sys/class/block") {
        for e in entries.flatten() {
            let pname = e.file_name().to_string_lossy().to_string();
            if !pname.starts_with(dev_name) || pname==dev_name { continue; }
            // Intentar leer la label de la particion via lsblk individual
            if let Ok(o) = Command::new("lsblk").args(["-n","-o","LABEL",&format!("/dev/{}",pname)]).output() {
                if String::from_utf8_lossy(&o.stdout).to_lowercase().trim().contains("ventoy") { return true; }
            }
        }
    }
    false
}

// ─── Buscar Ventoy2Disk.sh ────────────────────────────────────────────────────

fn find_ventoy_bin(send: &dyn Fn(f32,&str,LogLevel,bool)) -> Option<String> {
    // 0. PRIMERO: buscar en src/ventoy-1.1.12/ del propio proyecto
    let local_paths = [
        format!("{}/Ventoy2Disk.sh", VENTOY_LOCAL),
            format!("../src/ventoy-1.1.12/Ventoy2Disk.sh"),
                format!("./ventoy-1.1.12/Ventoy2Disk.sh"),
    ];
    for lp in &local_paths {
        if std::path::Path::new(lp).exists() {
            send(0.22, &format!("Ventoy encontrado en el proyecto: {}", lp), LogLevel::Info, false);
            return Some(lp.clone());
        }
    }
    // Buscar con find en src/ del proyecto
    if let Ok(o) = Command::new("find").args(["src","-name","Ventoy2Disk.sh","-maxdepth","3"])
        .stderr(std::process::Stdio::null()).output()
        {
            if let Some(line) = String::from_utf8_lossy(&o.stdout).lines().next().map(|l|l.trim().to_string()) {
                if !line.is_empty() {
                    send(0.22, &format!("Ventoy encontrado en src/: {}", line), LogLevel::Info, false);
                    return Some(line);
                }
            }
        }

    // 1. PATH del sistema
    if Command::new("which").arg("ventoy").output().map(|o|o.status.success()).unwrap_or(false) {
        if let Ok(o) = Command::new("which").arg("ventoy").output() {
            let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if !p.is_empty() { send(0.23,"ventoy encontrado en PATH",LogLevel::Info,false); return Some(p); }
        }
    }
    // 2. /opt/ventoy
    if std::path::Path::new("/opt/ventoy/Ventoy2Disk.sh").exists() {
        send(0.23,"Ventoy encontrado en /opt/ventoy",LogLevel::Info,false);
        return Some("/opt/ventoy/Ventoy2Disk.sh".into());
    }
    // 3. ~/Descargas, ~/Downloads, ~ (un nivel)
    let home = std::env::var("HOME").unwrap_or_else(|_|"/root".into());
    for base in [format!("{}/Descargas",&home), format!("{}/Downloads",&home), home.clone()] {
        if let Ok(entries) = std::fs::read_dir(&base) {
            for e in entries.flatten() {
                let fname = e.file_name().to_string_lossy().to_lowercase();
                if fname.starts_with("ventoy") && e.path().is_dir() {
                    let script = e.path().join("Ventoy2Disk.sh");
                    if script.exists() {
                        let s = script.to_string_lossy().to_string();
                        send(0.24,&format!("Ventoy encontrado: {}",s),LogLevel::Info,false);
                        return Some(s);
                    }
                }
            }
        }
    }
    // 4. find limitado
    send(0.25,"Buscando Ventoy2Disk.sh en el sistema (puede tardar 5s)...",LogLevel::Info,false);
    for root in [home.as_str(),"/tmp","/opt","/usr/local"] {
        if let Ok(o) = Command::new("timeout").args(["5","find",root,"-name","Ventoy2Disk.sh","-maxdepth","6"])
            .stderr(std::process::Stdio::null()).output()
            {
                if let Some(line) = String::from_utf8_lossy(&o.stdout).lines().next().map(|l|l.trim().to_string()) {
                    if !line.is_empty() { send(0.26,&format!("Encontrado: {}",line),LogLevel::Info,false); return Some(line); }
                }
            }
    }
    None
}

// ─── Red ──────────────────────────────────────────────────────────────────────

fn check_network() -> bool {
    Command::new("curl").args(["--silent","--max-time","3","--head","https://github.com"])
    .output().map(|o|o.status.success()).unwrap_or(false)
}

// Ejecuta un comando privilegiado (pkexec / sudo -A) respondiendo automaticamente
// "y" a cualquier prompt de confirmacion que el script pueda hacer por stdin
// (por ejemplo, la confirmacion de Ventoy2Disk.sh antes de escribir en el disco).
fn fetch_remote_catalog() -> Option<String> {
    let out = Command::new("curl").args(["--silent","--max-time","10","-L",CATALOG_URL]).output().ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8(out.stdout).ok()?;
    // Verificar que sea JSON valido con al menos una distro
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    if v["distros"].as_array().map(|a|a.len()).unwrap_or(0) == 0 { return None; }
    Some(s)
}

// ─── Descargas ────────────────────────────────────────────────────────────────

fn get_content_length(url: &str) -> u64 {
    Command::new("curl").args(["-sIL",url]).output().ok()
    .and_then(|o| {
        String::from_utf8_lossy(&o.stdout).lines()
        .find(|l| l.to_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1)?.trim().parse().ok())
    }).unwrap_or(0)
}

fn start_download(entry: &mut DownloadEntry, config: &AppConfig) {
    if entry.status == DownloadStatus::Downloading { return; }
    let url  = entry.url.clone();
    let dest = entry.dest_path.clone();
    let tmp  = format!("{}.part", dest);
    let rate = config.speed_limit.rate_arg().map(|s|s.to_string());

    entry.status    = DownloadStatus::Downloading;
    entry.progress  = 0.0;
    entry.speed_str = "Conectando...".into();

    if let Some(parent) = std::path::Path::new(&dest).parent() { let _ = std::fs::create_dir_all(parent); }

    let (tx, rx)         = channel::<DlProgress>();
    let (pause_tx, pause_rx) = channel::<()>();
    entry.progress_rx    = Some(rx);
    entry.pause_tx       = Some(pause_tx);

    std::thread::spawn(move || {
        let total = get_content_length(&url);
        let mut args = vec!["-q","-c","-O",&tmp,&url];
        let rate_s: String;
        if let Some(r) = &rate { rate_s = r.clone(); args.extend(["--limit-rate",&rate_s]); }
        let child = Command::new("wget").args(&args).spawn()
        .or_else(|_| {
            let mut ac = vec!["-L","-C","-","-o",&tmp,&url];
            if let Some(r) = &rate { ac.extend(["--limit-rate",r.as_str()]); }
            Command::new("curl").args(&ac).spawn()
        });
        let mut child = match child {
            Ok(c)  => c,
                       Err(e) => { let _ = tx.send(DlProgress{progress:0.0,speed:String::new(),done:true,error:Some(format!("Error: {}",e))}); return; }
        };
        let mut last_bytes = 0u64;
        let mut last_tick  = Instant::now();
        loop {
            // Comprobar si se pidio pausa
            if pause_rx.try_recv().is_ok() {
                let _ = child.kill();
                let _ = tx.send(DlProgress{progress:0.0,speed:String::new(),done:true,error:Some("__PAUSED__".into())});
                return;
            }
            std::thread::sleep(Duration::from_millis(800));
            let current  = std::fs::metadata(&tmp).map(|m|m.len()).unwrap_or(0);
            let dt       = last_tick.elapsed().as_secs_f64().max(0.1);
            let speed    = ((current.saturating_sub(last_bytes)) as f64 / dt) as u64;
            let progress = if total>0 { (current as f32/total as f32).min(0.99) } else { 0.0 };
            let _ = tx.send(DlProgress{progress, speed: format!("{}/s",format_size(speed)), done:false, error:None});
            last_bytes = current; last_tick = Instant::now();
            match child.try_wait() {
                Ok(Some(s)) => {
                    if s.success() {
                        let _ = std::fs::rename(&tmp,&dest);
                        let _ = tx.send(DlProgress{progress:1.0,speed:String::new(),done:true,error:None});
                    } else {
                        let _ = tx.send(DlProgress{progress:0.0,speed:String::new(),done:true,error:Some("Descarga fallida".into())});
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => { let _ = tx.send(DlProgress{progress:0.0,speed:String::new(),done:true,error:Some(e.to_string())}); break; }
            }
        }
    });
}

// Guardar/cargar estado de descargas
fn save_dl_state(downloads: &[DownloadEntry]) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let items: Vec<serde_json::Value> = downloads.iter()
    .filter(|d| !matches!(d.status, DownloadStatus::Done))
    .map(|d| serde_json::json!({
        "name": d.name, "url": d.url, "display_size": d.display_size,
        "dest_path": d.dest_path, "progress": d.progress,
        "status": match &d.status {
            DownloadStatus::Queued | DownloadStatus::Downloading | DownloadStatus::Paused => "paused",
            DownloadStatus::Error(_) => "error",
                               DownloadStatus::Done => "done",
        }
    })).collect();
    let _ = std::fs::write(format!("{}/downloads.json",&dir), serde_json::to_string_pretty(&items).unwrap_or_default());
}

fn load_dl_state() -> Vec<DownloadEntry> {
    let path = format!("{}/downloads.json", config_dir());
    let Ok(s) = std::fs::read_to_string(&path) else { return vec![] };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) else { return vec![] };
    v.as_array().unwrap_or(&vec![]).iter().filter_map(|d| {
        Some(DownloadEntry {
            name:         d["name"].as_str()?.to_string(),
             url:          d["url"].as_str()?.to_string(),
             display_size: d["display_size"].as_str().unwrap_or("").to_string(),
             dest_path:    d["dest_path"].as_str()?.to_string(),
             status:       DownloadStatus::Paused,
             progress:     d["progress"].as_f64().unwrap_or(0.0) as f32,
             speed_str:    String::new(),
             progress_rx:  None,
             pause_tx:     None,
        })
    }).collect()
}

fn scan_iso_files(dir: &str) -> Vec<IsoFile> {
    std::fs::read_dir(dir).ok().into_iter().flatten().flatten()
    .filter(|e| {
        let ext = e.path().extension().and_then(|x|x.to_str()).unwrap_or("").to_lowercase();
        ext=="iso"||ext=="img"
    })
    .map(|e| IsoFile {
        name: e.file_name().to_string_lossy().to_string(),
         path: e.path().to_string_lossy().to_string(),
         size_bytes: e.metadata().map(|m|m.len()).unwrap_or(0),
    }).collect()
}

// ─── Sidebar ──────────────────────────────────────────────────────────────────

fn sidebar_btn(ui: &mut egui::Ui, ctx: &egui::Context, panel: &mut Panel, th: &ThemeColors, target: Panel, icon: &str, label: &str, badge: bool) {
    let selected = *panel == target;
    let anim = ctx.animate_bool_with_time(egui::Id::new(format!("btn_{:?}",target)), selected, 0.18);
    let bg = lerp_color(th.sidebar_fill, Color32::from_rgb(40,80,180), anim);
    let fg = lerp_color(th.label_dim, Color32::WHITE, anim);
    let resp = ui.add(egui::Button::new(egui::RichText::new(format!("{icon}  {label}")).size(14.0).color(fg))
    .fill(bg).rounding(Rounding::same(8.0)).min_size(Vec2::new(150.0,38.0)));
    if resp.clicked() { *panel = target.clone(); }
    if anim>0.01 && anim<0.99 { ctx.request_repaint(); }
    if badge {
        let t  = ctx.input(|i|i.time) as f32;
        let pulse = ((t*3.0).sin()*0.3+0.7).clamp(0.0,1.0);
        let ba = ctx.animate_bool_with_time(egui::Id::new(format!("badge_{:?}",target)), badge, 0.35);
        if ba>0.01 {
            let pos   = resp.rect.right_top() + egui::vec2(-8.0,8.0);
            let alpha = (pulse*ba*255.0) as u8;
            ui.painter().circle_filled(pos, 5.0*ba, Color32::from_rgba_premultiplied(220,50,50,alpha));
            ui.painter().circle_stroke(pos, 5.0*ba, Stroke::new(1.5, Color32::from_rgba_premultiplied(255,100,100,alpha)));
        }
        ctx.request_repaint();
    }
}

// ─── Draw Dashboard ───────────────────────────────────────────────────────────

fn draw_dashboard(ui: &mut egui::Ui, usbs: &[UsbDevice], _scanning: bool, op_active: bool, op_cancel: bool, th: &ThemeColors, i18n: &HashMap<String,String>, action: &mut Option<DashAction>) {
    let tr = |k:&str| i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    if usbs.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("💾").size(48.0).color(th.icon_empty));
            ui.add_space(12.0);
            ui.label(egui::RichText::new(tr("dash_no_usb")).size(15.0).color(th.text));
            ui.add_space(6.0);
            ui.label(egui::RichText::new(tr("dash_auto_detect")).size(12.0).color(th.text_dim));
        });
        return;
    }
    let card_bg  = th.card_bg;
    let brd      = th.border;
    let badge_bg = th.badge_bg;
    let badge_fg = th.badge_fg;
    let path_col = th.text_path;
    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        for usb in usbs {
            let mut local: Option<DashAction> = None;
            Frame::none().fill(card_bg).rounding(Rounding::same(12.0)).stroke(Stroke::new(1.0,brd)).inner_margin(16.0)
            .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:12.0}).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🔌").size(28.0)); ui.add_space(8.0);
                    ui.vertical(|ui| {
                        let model_col = th.text_title;
                        ui.label(egui::RichText::new(&usb.model).size(15.0).strong().color(model_col));
                        ui.label(egui::RichText::new(&usb.path).size(12.0).color(path_col).monospace());
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        Frame::none().fill(badge_bg).rounding(Rounding::same(6.0))
                        .inner_margin(egui::Margin{left:10.0,right:10.0,top:4.0,bottom:4.0})
                        .show(ui, |ui| { ui.label(egui::RichText::new(format_size(usb.size_bytes)).size(12.0).color(badge_fg)); });
                        ui.add_space(8.0);
                        let (vbg,vtxt,vfg) = if usb.has_ventoy {
                            (Color32::from_rgb(20,80,40), "✓ Ventoy".to_string(), Color32::from_rgb(80,220,120))
                        } else {
                            (th.dash_no_ventoy_bg, tr("dash_without_ventoy"), th.dash_no_ventoy_fg)
                        };
                        Frame::none().fill(vbg).rounding(Rounding::same(6.0))
                        .inner_margin(egui::Margin{left:10.0,right:10.0,top:4.0,bottom:4.0})
                        .show(ui, |ui| { ui.label(egui::RichText::new(vtxt).size(12.0).color(vfg)); });
                    });
                });
                ui.add_space(12.0); ui.separator(); ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if op_active {
                        ui.spinner(); ui.add_space(6.0);
                        ui.label(egui::RichText::new(tr("dash_installing_ventoy")).size(13.0).color(Color32::from_rgb(80,140,255)));
                        ui.add_space(8.0);
                        if op_cancel {
                            if ui.add(egui::Button::new(egui::RichText::new(format!("✕  {}", tr("dash_cancel"))).size(12.0).color(Color32::from_rgb(220,80,80)))
                                .fill(Color32::from_rgb(60,20,20)).rounding(Rounding::same(7.0)).min_size(Vec2::new(100.0,30.0))).clicked() {
                                    local = Some(DashAction::CancelVentoy);
                                }
                        }
                    } else {
                        let (vtxt, is_upd) = if usb.has_ventoy { (format!("⬆  {}", tr("dash_update_ventoy")), true) } else { (format!("⚡  {}", tr("dash_install_ventoy")), false) };
                        if ui.add(egui::Button::new(egui::RichText::new(vtxt).size(13.0).color(Color32::WHITE))
                            .fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(7.0)).min_size(Vec2::new(165.0,32.0))).clicked() {
                                local = Some(DashAction::InstallVentoy(usb.path.clone(), is_upd));
                            }
                    }
                    ui.add_space(8.0);
                    if ui.add(egui::Button::new(egui::RichText::new(format!("🔥  {}", tr("dash_flash_iso"))).size(13.0).color(Color32::WHITE))
                        .fill(Color32::from_rgb(160,60,20)).rounding(Rounding::same(7.0)).min_size(Vec2::new(130.0,32.0))).clicked() {
                            local = Some(DashAction::GoFlash(usb.path.clone()));
                        }
                });
            });
            if local.is_some() { *action = local; }
        }
    });
}

// ─── Draw Catalogo ────────────────────────────────────────────────────────────

fn draw_catalog(ui: &mut egui::Ui, catalog: &[Distro], search: &mut String, filter: &mut CatFilter,
                win_popup: &mut bool, win_name: &mut String, downloads: &mut Vec<DownloadEntry>,
                config: &AppConfig, th: &ThemeColors, i18n: &HashMap<String,String>, catalog_updating: bool, catalog_resolving: bool,
                catalog_version: &str, catalog_updated: &str, catalog_refresh_trigger: &mut bool, go_downloads: &mut bool,
) {
    let tr = |k:&str| i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    ui.horizontal(|ui| {
        let sw = (ui.available_width()-130.0).max(320.0);
        ui.add(egui::TextEdit::singleline(search).hint_text(format!("🔍  {}", tr("cat_search_hint"))).desired_width(sw)
        .min_size(Vec2::new(0.0,36.0)).font(egui::FontId::proportional(15.0)));
        if !search.is_empty() {
            if ui.add(egui::Button::new(egui::RichText::new("✕").size(14.0)).min_size(Vec2::new(32.0,36.0))).clicked() { search.clear(); }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let lbl = tr("cat_distros_count").replace("{0}", &catalog.len().to_string());
            ui.label(egui::RichText::new(lbl).size(12.0).color(th.text_muted));
        });
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let hint = if catalog_updating { tr("catalog_updating") } else if catalog_resolving { tr("urls_resolving") } else { String::new() };
        if !hint.is_empty() {
            ui.label(egui::RichText::new(hint).size(11.0).color(th.text_muted).italics());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !catalog_version.is_empty() {
                let meta = format!("v{} · {}", catalog_version, catalog_updated);
                ui.label(egui::RichText::new(meta).size(11.0).color(th.text_dim).monospace());
                ui.add_space(8.0);
            }
            if catalog_resolving {
                ui.spinner();
                ui.add_space(4.0);
            }
            if ui.add(egui::Button::new(egui::RichText::new("🔗 URLs").size(11.0).color(th.label_dim))
                .fill(th.btn_clear_fill).rounding(Rounding::same(5.0)).min_size(Vec2::new(0.0,22.0)))
                .on_hover_text("Buscar URLs actualizadas para cada distribución")
                .clicked() {
                *catalog_refresh_trigger = true;
            }
            ui.add_space(4.0);
        });
    });
    ui.horizontal_wrapped(|ui| {
        for (f, key) in &[
            (CatFilter::All,"cat_filter_all"),
                          (CatFilter::Rolling,"cat_filter_rolling"),
                          (CatFilter::Lts,"cat_filter_lts"),
                          (CatFilter::NoSystemd,"cat_filter_nosystemd"),
                          (CatFilter::Server,"cat_filter_server"),
                          (CatFilter::Security,"cat_filter_security"),
                          (CatFilter::Gaming,"cat_filter_gaming"),
                          (CatFilter::Windows,"cat_filter_windows"),
        ] {
            let sel = *filter==*f;
            let bg = if sel { Color32::from_rgb(40,80,180) } else { th.bar_bg };
            let fg = if sel { Color32::WHITE } else { th.label_dim };
            let lbl = tr(*key);
            if ui.add(egui::Button::new(egui::RichText::new(lbl).size(12.0).color(fg)).fill(bg).rounding(Rounding::same(6.0)).min_size(Vec2::new(0.0,26.0))).clicked() { *filter=f.clone(); }
            ui.add_space(4.0);
        }
    });
    ui.add_space(14.0);
    let q = search.to_lowercase();
    let filtered: Vec<&Distro> = catalog.iter().filter(|d|
    (*filter==CatFilter::All || d.category==*filter) &&
    (q.is_empty() || d.name.to_lowercase().contains(&q) || d.description.to_lowercase().contains(&q) || d.arch.to_lowercase().contains(&q))
    ).collect();
    if filtered.is_empty() {
        ui.vertical_centered(|ui| { ui.add_space(40.0); ui.label(egui::RichText::new(tr("cat_no_results")).size(14.0).color(th.text)); });
        return;
    }
    let card_bg  = th.card_bg;
    let brd      = th.border;
    let desc_col = th.text_muted;
    let meta_col = th.text_muted;
    let name_col = th.text_title;
    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        let avail  = ui.available_width();
        let card_w = ((avail-16.0)/2.0).max(260.0);
        for chunk in filtered.chunks(2) {
            ui.horizontal(|ui| {
                for distro in chunk {
                    ui.vertical(|ui| {
                        ui.set_width(card_w);
                        let mut clicked = false;
                        Frame::none().fill(card_bg).rounding(Rounding::same(12.0)).stroke(Stroke::new(1.0,brd)).inner_margin(14.0).show(ui, |ui| {
                            ui.set_min_width(card_w-28.0);
                            ui.horizontal(|ui| {
                                // Logo con fallback a emoji
                                if !distro.logo.is_empty() {
                                    if let Some(uri) = logo_uri(&distro.logo) {
                                        ui.add(egui::Image::new(uri.as_str()).max_size(Vec2::new(32.0,32.0)).rounding(Rounding::same(4.0)));
                                    } else {
                                        ui.label(egui::RichText::new("💿").size(26.0));
                                    }
                                } else {
                                    ui.label(egui::RichText::new("💿").size(26.0));
                                }
                                ui.add_space(8.0);
                                ui.vertical(|ui| {
                                    // Nombre + arch badge
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(&distro.name).size(13.0).strong().color(name_col));
                                        if distro.arch == "i386" {
                                            Frame::none().fill(Color32::from_rgb(80,40,0)).rounding(Rounding::same(4.0))
                                            .inner_margin(egui::Margin{left:4.0,right:4.0,top:1.0,bottom:1.0}).show(ui, |ui| {
                                                ui.label(egui::RichText::new(tr("cat_32bit")).size(9.0).color(Color32::from_rgb(220,150,60)));
                                            });
                                        }
                                    });
                                    let (cbg,cfg,ctxt) = cat_badge(&distro.category, th, i18n);
                                    Frame::none().fill(cbg).rounding(Rounding::same(4.0))
                                    .inner_margin(egui::Margin{left:6.0,right:6.0,top:2.0,bottom:2.0}).show(ui, |ui| {
                                        ui.label(egui::RichText::new(&ctxt).size(10.0).color(cfg));
                                    });
                                });
                            });
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(&distro.description).size(12.0).color(desc_col));
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("💾 {} MB",distro.size_mb)).size(11.0).color(meta_col));
                                ui.add_space(10.0);
                                ui.label(egui::RichText::new(format!("⚙ {}",distro.arch)).size(11.0).color(meta_col));
                            });
                            if distro.is_windows {
                                ui.add_space(6.0);
                                Frame::none().fill(Color32::from_rgb(60,40,10)).rounding(Rounding::same(6.0))
                                .inner_margin(egui::Margin{left:8.0,right:8.0,top:5.0,bottom:5.0}).show(ui, |ui| {
                                    ui.label(egui::RichText::new(format!("⚠  {}", tr("cat_special_download_warning"))).size(11.0).color(Color32::from_rgb(230,170,60)));
                                });
                            }
                            ui.add_space(10.0); ui.separator(); ui.add_space(8.0);
                            let in_queue = downloads.iter().any(|d|d.url==distro.url);
                            let (btn_col,btn_txt) = if distro.is_windows {
                                (Color32::from_rgb(0,90,190), format!("🪟  {}", tr("cat_view_instructions")))
                            } else if in_queue {
                                (Color32::from_rgb(30,80,40), format!("✓  {}", tr("cat_in_queue")))
                            } else {
                                (Color32::from_rgb(40,80,180), format!("⬇  {}", tr("cat_add_to_downloads")))
                            };
                            if ui.add(egui::Button::new(egui::RichText::new(btn_txt).size(12.0).color(Color32::WHITE)).fill(btn_col).rounding(Rounding::same(7.0)).min_size(Vec2::new(ui.available_width(),30.0))).clicked() {
                                clicked = true;
                            }
                        });
                        if clicked {
                            if distro.is_windows { *win_popup = true; *win_name = distro.name.clone(); }
                            else if !downloads.iter().any(|d|d.url==distro.url) {
                                downloads.push(DownloadEntry {
                                    name: distro.name.clone(), url: distro.url.clone(),
                                               display_size: format!("{} MB",distro.size_mb),
                                               dest_path: format!("{}/{}.iso", config.download_dir, safe_name(&distro.name)),
                                               status: DownloadStatus::Queued, progress: 0.0, speed_str: String::new(),
                                               progress_rx: None, pause_tx: None,
                                });
                                *go_downloads = true;
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

// ─── Draw Descargas ───────────────────────────────────────────────────────────

fn draw_descargas(ui: &mut egui::Ui, downloads: &mut Vec<DownloadEntry>, config: &AppConfig, th: &ThemeColors, i18n: &HashMap<String,String>) -> Option<DlAction> {
    let tr = |k:&str| i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    let card_bg  = th.card_bg;
    let brd      = th.border;
    let name_col = th.text_title;
    let url_col  = th.text_muted;
    let tc       = th.text;
    let bar_bg_c = th.bar_bg;

    if downloads.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("⬇").size(48.0).color(th.icon_empty)); ui.add_space(12.0);
            ui.label(egui::RichText::new(tr("dl_no_downloads")).size(15.0).color(tc)); ui.add_space(6.0);
            ui.label(egui::RichText::new(tr("dl_go_to_catalog")).size(12.0).color(th.text_dim));
        });
        return None;
    }

    // Info bar
    Frame::none().fill(bar_bg_c).rounding(Rounding::same(8.0)).inner_margin(10.0).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let dir_valid = validate_download_dir(&config.download_dir, i18n).is_ok();
            let dir_col   = if dir_valid { url_col } else { Color32::from_rgb(220,80,80) };
            ui.label(egui::RichText::new("📁").size(13.0));
            ui.label(egui::RichText::new(&config.download_dir).size(12.0).color(dir_col).monospace());
            if !dir_valid {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(format!("⚠ {}", tr("dl_invalid_path"))).size(11.0).color(Color32::from_rgb(220,80,80)));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new(format!("⚡ {}", config.speed_limit.label(i18n))).size(12.0).color(tc));
            });
        });
    });
    ui.add_space(8.0);

    let dl_dir_ok = validate_download_dir(&config.download_dir, i18n).is_ok();

    let clear_btn_fill = th.btn_clear_fill;
    let clear_btn_fg  = th.btn_clear_fg;

    ui.horizontal(|ui| {
        let lbl = tr("dl_items_count").replace("{0}", &downloads.len().to_string());
        ui.label(egui::RichText::new(lbl).size(13.0).color(tc));
    });
    ui.add_space(8.0);

    let mut action: Option<DlAction> = None;
    let mut clear_done = false;

    egui::ScrollArea::vertical().max_height(ui.available_height()-40.0).show(ui, |ui| {
        for (i, dl) in downloads.iter().enumerate() {
            Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(14.0)
            .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:8.0}).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    let sico = match &dl.status {
                        DownloadStatus::Queued      => "🕐",
                        DownloadStatus::Downloading => "⬇",
                        DownloadStatus::Paused      => "⏸",
                        DownloadStatus::Done        => "✅",
                        DownloadStatus::Error(_)    => "❌",
                    };
                    ui.label(egui::RichText::new(sico).size(22.0)); ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&dl.name).size(14.0).strong().color(name_col));
                        let boundary = dl.url.char_indices().nth(55).map(|(i,_)| i).unwrap_or(dl.url.len());
                        let url_s = &dl.url[..boundary];
                        ui.label(egui::RichText::new(format!("💾 {}  •  {}...", dl.display_size, url_s)).size(11.0).color(url_col).monospace());
                        if dl.status==DownloadStatus::Downloading && !dl.speed_str.is_empty() {
                            let pct = (dl.progress*100.0) as u32;
                            ui.label(egui::RichText::new(format!("{}%  —  {}",pct,dl.speed_str)).size(11.0).color(Color32::from_rgb(80,180,120)));
                        }
                        if dl.status==DownloadStatus::Paused && dl.progress>0.0 {
                            let pct = (dl.progress*100.0) as u32;
                            ui.label(egui::RichText::new(format!("{}%  —  {}",pct, tr("dl_paused_resume"))).size(11.0).color(Color32::from_rgb(200,160,60)));
                        }
                        if let DownloadStatus::Error(e) = &dl.status {
                            ui.label(egui::RichText::new(format!("{}: {}", tr("dl_error"), e)).size(11.0).color(Color32::from_rgb(220,80,80)));
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(egui::RichText::new("✕").size(12.0)).fill(Color32::TRANSPARENT)).clicked() {
                            action = Some(DlAction::Remove(i));
                        }
                        ui.add_space(4.0);
                        match &dl.status {
                            DownloadStatus::Done => {
                                if ui.add(egui::Button::new(egui::RichText::new(format!("📁 {}", tr("dl_open"))).size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(30,80,40)).rounding(Rounding::same(6.0)).min_size(Vec2::new(80.0,28.0))).clicked() {
                                    action = Some(DlAction::OpenDir(i));
                                }
                            }
                            DownloadStatus::Queued | DownloadStatus::Paused | DownloadStatus::Error(_) => {
                                if dl_dir_ok {
                                    let lbl = if dl.status==DownloadStatus::Paused { format!("▶  {}", tr("dl_resume")) } else { format!("▶  {}", tr("dl_start")) };
                                    if ui.add(egui::Button::new(egui::RichText::new(lbl).size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(6.0)).min_size(Vec2::new(100.0,28.0))).clicked() {
                                        action = Some(DlAction::Start(i));
                                    }
                                }
                            }
                            DownloadStatus::Downloading => {
                                if ui.add(egui::Button::new(egui::RichText::new(format!("⏸  {}", tr("dl_pause"))).size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(100,70,20)).rounding(Rounding::same(6.0)).min_size(Vec2::new(90.0,28.0))).clicked() {
                                    action = Some(DlAction::Pause(i));
                                }
                            }
                        }
                    });
                });
                if dl.status==DownloadStatus::Downloading || (dl.status==DownloadStatus::Paused && dl.progress>0.0) {
                    ui.add_space(8.0);
                    let bw = ui.available_width()-4.0;
                    let (rect,_) = ui.allocate_exact_size(Vec2::new(bw,8.0), egui::Sense::hover());
                    let pbg = th.bar_bg;
                    ui.painter().rect_filled(rect, Rounding::same(4.0), pbg);
                    if dl.progress>0.0 {
                        let fw   = rect.width()*dl.progress;
                        let fill = egui::Rect::from_min_size(rect.min, Vec2::new(fw,rect.height()));
                        let col  = if dl.status==DownloadStatus::Paused { Color32::from_rgb(160,120,40) } else { Color32::from_rgb(40,100,220) };
                        ui.painter().rect_filled(fill, Rounding::same(4.0), col);
                    }
                }
            });
        }
    });

    if ui.add(egui::Button::new(egui::RichText::new(format!("🗑  {}", tr("dl_clear_completed"))).size(12.0).color(clear_btn_fg))
        .fill(clear_btn_fill).rounding(Rounding::same(6.0))).clicked() {
            clear_done = true;
        }
        if clear_done { action = Some(DlAction::ClearDone); }
        action
}

// ─── Draw ISOs Locales ────────────────────────────────────────────────────────

fn draw_flasheo(
    ui: &mut egui::Ui, usbs: &[UsbDevice], local_isos: &[IsoFile], downloads: &[DownloadEntry],
    target_usb: &mut Option<String>, target_iso: &mut Option<String>, th: &ThemeColors, i18n: &HashMap<String,String>, op_active: bool
) -> Option<(String, String)> {
    let tr = |k:&str| i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    let mut start_flash = None;
    let card_bg  = th.card_bg;
    let brd      = th.border;
    let tc       = th.text;
    let title_col= th.text_title;
    
    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        // --- SELECCIONAR USB ---
        ui.label(egui::RichText::new(tr("nav_dashboard")).size(14.0).strong().color(tc));
        ui.add_space(8.0);
        let ventoy_usbs: Vec<_> = usbs.iter().filter(|u| u.has_ventoy).collect();
        if ventoy_usbs.is_empty() {
            Frame::none().fill(card_bg).rounding(Rounding::same(8.0)).stroke(Stroke::new(1.0,brd)).inner_margin(12.0).show(ui, |ui| {
                ui.label(egui::RichText::new("⚠️ No se encontraron dispositivos USB con Ventoy instalado.").color(Color32::from_rgb(200,100,50)));
            });
        } else {
            for usb in ventoy_usbs {
                let is_selected = target_usb.as_ref() == Some(&usb.path);
                let bg = if is_selected { Color32::from_rgb(40,60,100) } else { card_bg };
                let st = if is_selected { Stroke::new(2.0, Color32::from_rgb(80,140,255)) } else { Stroke::new(1.0, brd) };
                
                let resp = Frame::none().fill(bg).rounding(Rounding::same(8.0)).stroke(st).inner_margin(12.0)
                .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:8.0}).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🔌").size(20.0)); ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&usb.model).size(14.0).strong().color(title_col));
                            ui.label(egui::RichText::new(&usb.path).size(11.0).color(tc).monospace());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format_size(usb.size_bytes)).size(12.0).color(tc));
                        });
                    });
                });
                if resp.response.interact(egui::Sense::click()).clicked() {
                    *target_usb = Some(usb.path.clone());
                }
            }
        }
        ui.add_space(20.0);
        
        // --- SELECCIONAR ISO ---
        ui.label(egui::RichText::new(tr("nav_local_isos")).size(14.0).strong().color(tc));
        ui.add_space(8.0);
        let mut all_isos = Vec::new();
        for iso in local_isos { all_isos.push((iso.name.clone(), iso.path.clone(), iso.size_bytes)); }
        for dl in downloads {
            if dl.status == DownloadStatus::Done {
                if let Ok(m) = std::fs::metadata(&dl.dest_path) {
                    all_isos.push((dl.name.clone(), dl.dest_path.clone(), m.len()));
                }
            }
        }
        
        if all_isos.is_empty() {
            Frame::none().fill(card_bg).rounding(Rounding::same(8.0)).stroke(Stroke::new(1.0,brd)).inner_margin(12.0).show(ui, |ui| {
                ui.label(egui::RichText::new("💿 No hay archivos ISO disponibles localmente ni descargados.").color(tc));
            });
        } else {
            for (name, path, size) in all_isos {
                let is_selected = target_iso.as_ref() == Some(&path);
                let bg = if is_selected { Color32::from_rgb(40,60,100) } else { card_bg };
                let st = if is_selected { Stroke::new(2.0, Color32::from_rgb(80,140,255)) } else { Stroke::new(1.0, brd) };
                
                let resp = Frame::none().fill(bg).rounding(Rounding::same(8.0)).stroke(st).inner_margin(12.0)
                .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:8.0}).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("💿").size(20.0)); ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&name).size(14.0).strong().color(title_col));
                            ui.label(egui::RichText::new(&path).size(11.0).color(tc).monospace());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(format_size(size)).size(12.0).color(tc));
                        });
                    });
                });
                if resp.response.interact(egui::Sense::click()).clicked() {
                    *target_iso = Some(path.clone());
                }
            }
        }
        ui.add_space(30.0);
        
        // --- BOTON FLASHEAR ---
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 100.0);
            let btn_color = if op_active { Color32::from_rgb(80,80,90) } else { Color32::from_rgb(220,60,60) };
            let btn = egui::Button::new(
                egui::RichText::new("⚡ Flashear a Ventoy")
                .size(16.0).strong().color(Color32::WHITE)
            ).fill(btn_color).rounding(Rounding::same(8.0)).min_size(Vec2::new(200.0, 45.0));
            
            ui.add_enabled_ui(target_usb.is_some() && target_iso.is_some() && !op_active, |ui| {
                if ui.add(btn).clicked() {
                    if let (Some(u), Some(i)) = (target_usb.clone(), target_iso.clone()) {
                        start_flash = Some((u, i));
                    }
                }
            });
        });
        ui.add_space(20.0);
    });
    start_flash
}

fn draw_locales(ui: &mut egui::Ui, iso_files: &[IsoFile], scan_dir: &str, th: &ThemeColors, i18n: &HashMap<String,String>) -> bool {
    let tr = |k:&str| i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    let card_bg  = th.card_bg;
    let brd      = th.border;
    let name_col = th.text_title;
    let path_col = th.text_muted;
    let tc       = th.text;
    let mut rescan = false;
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("📁  {}",scan_dir)).size(13.0).color(path_col).monospace());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new(egui::RichText::new(format!("🔄  {}", tr("local_update"))).size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(7.0)).min_size(Vec2::new(100.0,28.0))).clicked() { rescan=true; }
        });
    });
    ui.add_space(10.0);
    if iso_files.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.label(egui::RichText::new("💿").size(44.0).color(th.icon_empty)); ui.add_space(12.0);
            ui.label(egui::RichText::new(tr("local_no_files")).size(15.0).color(tc)); ui.add_space(6.0);
            let t2 = th.text_dim;
            ui.label(egui::RichText::new(tr("local_go_to_catalog_or_config")).size(12.0).color(t2));
        });
    } else {
        let lbl = tr("local_files_count").replace("{0}", &iso_files.len().to_string());
        ui.label(egui::RichText::new(lbl).size(13.0).color(tc));
        ui.add_space(8.0);
        egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
            for iso in iso_files {
                Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(14.0)
                .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:8.0}).show(ui, |ui| {
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
                                if let Some(parent) = std::path::Path::new(&iso.path).parent() {
                                    let _ = Command::new("xdg-open").arg(parent).spawn();
                                }
                            }
                        });
                    });
                });
            }
        });
    }
    rescan
}

// ─── Draw Configuracion ───────────────────────────────────────────────────────

fn draw_configuracion(ui: &mut egui::Ui, config: &mut AppConfig, suggestions: &[String], show_sug: &mut bool, th: &ThemeColors, i18n: &HashMap<String,String>, lang: &mut Language, on_lang_change: &mut bool) {
    let tr = |k:&str| i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    let sec_col  = th.section_title;
    let tc       = th.text;
    let card_bg  = th.card_bg;
    let brd      = th.border;
    let sug_bg   = th.sug_bg;

    egui::ScrollArea::vertical().max_height(ui.available_height()).show(ui, |ui| {
        // ── Directorio de descargas ──
        Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(16.0)
        .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:14.0}).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(format!("📁  {}", tr("cfg_download_dir"))).size(14.0).strong().color(sec_col));
            ui.add_space(8.0);
            let dir_valid = validate_download_dir(&config.download_dir, i18n);
            let border_col = if dir_valid.is_ok() { brd } else { Color32::from_rgb(200,60,60) };
            // Campo de texto
            let resp = ui.add(egui::TextEdit::singleline(&mut config.download_dir)
            .desired_width(ui.available_width()-110.0)
            .hint_text("/home/usuario/Descargas")
            .text_color(if dir_valid.is_ok() { th.text_title } else { Color32::from_rgb(220,80,80) }));
            if resp.changed() { *show_sug = true; }
            // Boton crear
            ui.horizontal(|ui| {
                if let Err(e) = dir_valid {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(&e).size(11.0).color(Color32::from_rgb(220,80,80)));
                    if e.contains("no existe") || e.contains("not exist") || e.contains("不存在") {
                        ui.add_space(8.0);
                        if ui.add(egui::Button::new(egui::RichText::new(format!("✚ {}", tr("cfg_create"))).size(12.0).color(Color32::WHITE))
                            .fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(6.0))).clicked() {
                                let _ = std::fs::create_dir_all(&config.download_dir);
                            }
                    }
                }
            });
            // Sugerencias de autocompletado
            if *show_sug && !suggestions.is_empty() {
                ui.add_space(4.0);
                Frame::none().fill(sug_bg).rounding(Rounding::same(6.0)).stroke(Stroke::new(1.0,border_col))
                .inner_margin(6.0).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    for s in suggestions {
                        let label_col = th.text_title;
                        if ui.add(egui::Button::new(egui::RichText::new(format!("📁 {}",s)).size(12.0).color(label_col))
                            .fill(Color32::TRANSPARENT).rounding(Rounding::same(4.0)).min_size(Vec2::new(ui.available_width()-12.0,24.0))).clicked() {
                                config.download_dir = s.clone();
                                *show_sug = false;
                            }
                    }
                    if ui.add(egui::Button::new(egui::RichText::new(format!("✕  {}", tr("cfg_close_suggestions"))).size(11.0).color(tc))
                        .fill(Color32::TRANSPARENT)).clicked() { *show_sug = false; }
                });
            }
        });

        // ── Velocidad ──
        Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(16.0)
        .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:14.0}).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(format!("⚡  {}", tr("cfg_speed"))).size(14.0).strong().color(sec_col));
            ui.add_space(8.0);
            ui.label(egui::RichText::new(tr("cfg_speed_desc")).size(12.0).color(tc));
            ui.add_space(12.0);
            for variant in [SpeedLimit::Low, SpeedLimit::Medium, SpeedLimit::High, SpeedLimit::Max] {
                let sel = config.speed_limit == variant;
                let bg  = if sel { Color32::from_rgb(40,80,180) } else { th.btn_clear_fill };
                let fg  = if sel { Color32::WHITE } else { th.label_dim };
                if ui.add(egui::Button::new(egui::RichText::new(variant.label(i18n)).size(13.0).color(fg))
                    .fill(bg).rounding(Rounding::same(7.0)).min_size(Vec2::new(280.0,32.0))).clicked() {
                        config.speed_limit = variant;
                    }
                    ui.add_space(4.0);
            }
        });

        // ── Idioma ──
        Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(16.0)
        .outer_margin(egui::Margin{left:0.0,right:0.0,top:0.0,bottom:14.0}).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(format!("🌐  {}", tr("cfg_language"))).size(14.0).strong().color(sec_col));
            ui.add_space(10.0);
            for (lbl_key, l) in [
                ("cfg_lang_english", Language::English),
                                                                                 ("cfg_lang_spanish", Language::Spanish),
                                                                                 ("cfg_lang_chinese", Language::Chinese),
            ] {
                let sel = *lang == l;
                let bg  = if sel { Color32::from_rgb(40,80,180) } else { th.btn_clear_fill };
                let fg  = if sel { Color32::WHITE } else { th.label_dim };
                if ui.add(egui::Button::new(egui::RichText::new(tr(lbl_key)).size(13.0).color(fg))
                    .fill(bg).rounding(Rounding::same(7.0)).min_size(Vec2::new(280.0,32.0))).clicked() {
                        *lang = l;
                        *on_lang_change = true;
                    }
                    ui.add_space(4.0);
            }
        });

        // ── Logos ──
        Frame::none().fill(card_bg).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,brd)).inner_margin(16.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(egui::RichText::new(format!("🖼  {}", tr("cfg_logos"))).size(14.0).strong().color(sec_col));
            ui.add_space(8.0);
            ui.label(egui::RichText::new(tr("cfg_logos_desc")).size(12.0).color(tc));
            ui.add_space(8.0);
            let logos = ["almalinux.svg","alpine.svg","antix.svg","arch.svg","bazzite.svg","cachyos.svg","debian.svg","deepin.svg","endeavouros.svg","fedora.svg","kali.svg","kde.svg","kubuntu.svg","lubuntu.svg","manjaro.svg","mint.svg","mxlinux.svg","nixos.svg","nobara.svg","opensuse.svg","parrot.svg","popos.svg","q4os.svg","rocky.svg","slackware.svg","tails.svg","ubuntu.svg","ubuntubudgie.svg","ubuntucinnamon.svg","ubuntustudio.svg","void.svg","windows.svg","xubuntu.svg","zorin.svg"];
            ui.horizontal_wrapped(|ui| {
                for logo in logos {
                    let exists = std::path::Path::new("icons").join(logo).exists();
                    let (bg,fg) = if exists { (Color32::from_rgb(20,70,30), Color32::from_rgb(80,220,100)) } else {
                        (th.btn_clear_fill, th.text)
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

// ─── Draw Logs ────────────────────────────────────────────────────────────────

fn draw_logs(ui: &mut egui::Ui, ctx: &egui::Context, op: &mut OpProgress, th: &ThemeColors, i18n: &HashMap<String,String>) {
    let tr = |k:&str| i18n.get(k).cloned().unwrap_or_else(|| k.to_string());
    let t = ctx.input(|i|i.time) as f32;
    if op.active {
        let dots = ".".repeat((t*2.0) as usize % 4);
        ui.label(egui::RichText::new(format!("⚡ {}{}",&op.label,dots)).size(15.0).strong().color(Color32::from_rgb(80,140,255)));
        ui.add_space(12.0);
        let pct = (op.progress*100.0) as u32;
        let bw  = ui.available_width()-20.0;
        let (rect,_) = ui.allocate_exact_size(Vec2::new(bw,28.0), egui::Sense::hover());
        let p   = ui.painter();
        let pbg = th.bar_bg;
        p.rect_filled(rect, Rounding::same(8.0), pbg);
        if op.progress>0.0 {
            let fw = rect.width()*op.progress;
            let fr = egui::Rect::from_min_size(rect.min, Vec2::new(fw,rect.height()));
            p.rect_filled(fr, Rounding::same(8.0), Color32::from_rgb(30,80,200));
            let sh = egui::Rect::from_min_size(rect.min, Vec2::new(fw,rect.height()/2.0));
            p.rect_filled(sh, Rounding{nw:8.0,ne:8.0,sw:0.0,se:0.0}, Color32::from_rgba_premultiplied(80,140,255,60));
        }
        let bc = th.text_muted;
        p.rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0,bc));
        let pc = th.text_title;
        p.text(rect.center(), egui::Align2::CENTER_CENTER, format!("{}%",pct), egui::FontId::proportional(13.0), pc);
        ui.add_space(10.0);
        if let Some(last) = op.logs.last() {
            ui.label(egui::RichText::new(format!("  {}",&last.message)).size(12.0).color(th.text).italics());
        }
        ui.add_space(16.0);
        if op.cancel_tx.is_some() {
            if ui.add(egui::Button::new(egui::RichText::new("✕  Cancelar").size(13.0).color(Color32::from_rgb(220,80,80)))
                .fill(Color32::from_rgb(60,20,20)).rounding(Rounding::same(7.0)).min_size(Vec2::new(120.0,30.0))).clicked() {
                    let tx = op.cancel_tx.take().unwrap();
                    let _ = tx.send(()); op.active = false;
                    op.log(&tr("logs_cancelled_by_user"), LogLevel::Warn);
                }
        }
    } else if !op.logs.is_empty() {
        let ok  = op.logs.iter().any(|l|l.level==LogLevel::Ok);
        let err = op.logs.iter().any(|l|l.level==LogLevel::Error);
        let (icon,txt,col) = if ok&&!err { ("✅", tr("logs_completed"), Color32::from_rgb(80,200,120)) }
        else if err { ("❌", tr("logs_with_errors"), Color32::from_rgb(220,80,80)) }
        else { ("⚠", tr("logs_cancelled"), Color32::from_rgb(220,180,60)) };
        ui.label(egui::RichText::new(format!("{icon}  {txt}")).size(15.0).strong().color(col));
        ui.add_space(12.0);
    } else {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            ui.label(egui::RichText::new("📋").size(40.0).color(th.icon_empty)); ui.add_space(10.0);
            ui.label(egui::RichText::new(tr("logs_idle")).size(14.0).color(th.text));
        }); return;
    }
    if !op.logs.is_empty() {
        let tog = if op.logs_expanded { format!("▼  {}", tr("logs_hide_details")) } else { format!("▶  {}", tr("logs_view_details")) };
        if ui.add(egui::Button::new(egui::RichText::new(tog).size(13.0).color(Color32::from_rgb(100,140,220))).fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
            op.logs_expanded = !op.logs_expanded;
        }
        let anim = ctx.animate_bool_with_time(egui::Id::new("logs_expand"), op.logs_expanded, 0.20);
        if anim>0.01 { ctx.request_repaint(); }
        if anim>0.01 {
            ui.add_space(8.0);
            let lb   = th.panel_fill;
            let lbrd = th.border;
            Frame::none().fill(lb).rounding(Rounding::same(10.0)).stroke(Stroke::new(1.0,lbrd)).inner_margin(12.0).show(ui, |ui| {
                ui.set_min_width(ui.available_width()-20.0);
                egui::ScrollArea::vertical().max_height(200.0*anim).stick_to_bottom(true).show(ui, |ui| {
                    for e in &op.logs {
                        let (pre,col) = match e.level {
                            LogLevel::Info  => ("INFO ", Color32::from_rgb(160,170,190)),
                                                                                               LogLevel::Ok    => ("OK   ", Color32::from_rgb(80,200,120)),
                                                                                               LogLevel::Warn  => ("WARN ", Color32::from_rgb(220,180,60)),
                                                                                               LogLevel::Error => ("ERR  ", Color32::from_rgb(220,80,80)),
                        };
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("[{}]",&e.timestamp)).size(11.0).monospace().color(Color32::from_rgb(90,95,115)));
                            ui.label(egui::RichText::new(pre).size(11.0).monospace().color(col));
                            ui.label(egui::RichText::new(&e.message).size(11.0).monospace().color(th.text));
                        });
                    }
                });
            });
        }
        ui.add_space(12.0);
        if !op.active {
            if ui.add(egui::Button::new(egui::RichText::new(format!("🗑  {}", tr("logs_clear"))).size(12.0).color(Color32::from_rgb(180,80,80))).fill(Color32::TRANSPARENT).rounding(Rounding::same(6.0))).clicked() {
                op.logs.clear(); op.logs_expanded = false;
            }
        }
    }
}

// ─── App ──────────────────────────────────────────────────────────────────────

struct IsoFlash {
    panel: Panel, tema: Tema, tema_anim: f32,
    usbs: Vec<UsbDevice>, scanning: bool, last_scan: f64,
    usb_rx: Option<Receiver<Vec<UsbDevice>>>,
    rescan_after: Option<f64>,
    op: OpProgress, op_rx: Option<Receiver<(f32,String,LogLevel,bool)>>,
    catalog: Vec<Distro>,
    catalog_rx: Option<Receiver<Option<String>>>,
    catalog_updating: bool,
    last_catalog_update: f64,
    catalog_hash: u64,
    catalog_version: String,
    catalog_updated: String,
    catalog_resolve_rx: Option<Receiver<Option<Vec<Distro>>>>,
    catalog_resolve_requested: bool,
    has_network: bool, network_rx: Option<Receiver<bool>>, last_net_check: f64,
    cat_search: String, cat_filter: CatFilter, cat_win_popup: bool, cat_win_name: String,
    downloads: Vec<DownloadEntry>,
    iso_files: Vec<IsoFile>,
    config: AppConfig,
    lang: Language,
    i18n: HashMap<String, String>,
    notif: Option<(String, f64)>,
    show_path_sug: bool, path_sug: Vec<String>,
    flash_target_usb: Option<String>,
    flash_target_iso: Option<String>,
    #[allow(dead_code)]
    first_init: bool,
}

impl Default for IsoFlash {
    fn default() -> Self {
        let catalog = load_catalog(CATALOG_JSON);
        let catalog_hash = hash_str(CATALOG_JSON);
        let downloads = load_dl_state();
        let (lang, download_dir, speed_limit, first_init) = {
            if let Some((l, d, sp)) = load_app_config() {
                let sl = match sp.as_str() { "Low"=>SpeedLimit::Low, "Medium"=>SpeedLimit::Medium, "High"=>SpeedLimit::High, _=>SpeedLimit::Max };
                // Si hay config guardada, usar eso (primer arranque -> false)
                (l, d, sl, false)
            } else {
                // Primer arranque: detectar idioma del sistema
                let l = Language::default();
                let d = default_download_dir();
                (l, d, SpeedLimit::default(), true)
            }
        };
        let download_dir = if download_dir.is_empty() { default_download_dir() } else { download_dir };
        let i18n = load_i18n(lang);
        let has_catalog = !catalog.is_empty();
        Self {
            panel: Panel::Dashboard, tema: Tema::Oscuro, tema_anim: 0.0,
            usbs: vec![], scanning: false, last_scan: -999.0, usb_rx: None,
            rescan_after: None,
            op: OpProgress::default(), op_rx: None,
            catalog, catalog_rx: None, catalog_updating: false, last_catalog_update: -9999.0, catalog_hash,
            catalog_version: String::new(), catalog_updated: String::new(),
            catalog_resolve_rx: None, catalog_resolve_requested: has_catalog,
            has_network: false, network_rx: None, last_net_check: -999.0,
            cat_search: String::new(), cat_filter: CatFilter::All,
            cat_win_popup: false, cat_win_name: String::new(),
            downloads, iso_files: vec![], config: AppConfig { download_dir, speed_limit },
            lang, i18n,
            notif: None, show_path_sug: false, path_sug: vec![],
            flash_target_usb: None, flash_target_iso: None,
            first_init,
        }
    }
}

impl IsoFlash {
    fn t(&self, key: &str) -> String {
        self.i18n.get(key).cloned().unwrap_or_else(|| key.to_string())
    }
    fn tf(&self, key: &str, args: &[&str]) -> String {
        let tmpl = self.t(key);
        let mut result = tmpl;
        for (i, arg) in args.iter().enumerate() {
            result = result.replace(&format!("{{{}}}", i), arg);
        }
        result
    }

    fn start_flash_iso(&mut self, usb_path: String, iso_path: String) {
        if self.op.active { return; }
        self.op = OpProgress::default();
        self.op.active = true;
        self.op.label = format!("Flasheando ISO a {}", usb_path);
        self.op.log(&format!("Preparando flasheo de {} a {}", iso_path, usb_path), LogLevel::Info);
        
        let (tx, rx) = channel::<(f32, String, LogLevel, bool)>();
        let (ctx, crx) = channel::<()>();
        self.op.cancel_tx = Some(ctx);
        self.op_rx = Some(rx);
        
        std::thread::spawn(move || {
            let cancelled = || crx.try_recv().is_ok();
            let send = |p:f32, msg:&str, lvl:LogLevel, done:bool| { let _ = tx.send((p,msg.to_string(),lvl,done)); };
            
            // 1. Encontrar particion de Ventoy (usualmente particion 1)
            let dev_name = usb_path.trim_start_matches("/dev/");
            let mut ventoy_part = None;
            if let Ok(entries) = std::fs::read_dir("/sys/class/block") {
                for e in entries.flatten() {
                    let pname = e.file_name().to_string_lossy().to_string();
                    if !pname.starts_with(dev_name) || pname == dev_name { continue; }
                    if let Ok(o) = Command::new("lsblk").args(["-n","-o","LABEL",&format!("/dev/{}", pname)]).output() {
                        if String::from_utf8_lossy(&o.stdout).to_lowercase().trim() == "ventoy" {
                            ventoy_part = Some(format!("/dev/{}", pname));
                            break;
                        }
                    }
                }
            }
            let part_path = match ventoy_part {
                Some(p) => p,
                None => {
                    // Fallback: assume partition 1
                    format!("{}1", usb_path)
                }
            };
            
            send(0.02, &format!("Usando particion Ventoy: {}", part_path), LogLevel::Info, false);
            
            // 2. Montar particion si no esta montada
            let mut mount_point = None;
            
            if let Ok(o) = Command::new("lsblk").args(["-n","-o","MOUNTPOINT",&part_path]).output() {
                let mp = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !mp.is_empty() { mount_point = Some(mp); }
            }
            
            if mount_point.is_none() {
                send(0.05, "Montando particion de Ventoy...", LogLevel::Info, false);
                let mo = Command::new("udisksctl").args(["mount", "-b", &part_path]).output();
                if let Ok(o) = mo {
                    let out = String::from_utf8_lossy(&o.stdout);
                    if let Some(idx) = out.find(" at ") {
                        let mp = out[idx+4..].trim().trim_end_matches('.');
                        mount_point = Some(mp.to_string());
                    }
                }
            }
            
            let dest_dir = match mount_point {
                Some(p) => p,
                None => {
                    send(1.0, "Error: No se pudo montar la particion de Ventoy", LogLevel::Error, true);
                    return;
                }
            };
            
            send(0.1, &format!("Particion montada en: {}", dest_dir), LogLevel::Ok, false);
            
            // 3. Validar espacio libre
            let file_size = match std::fs::metadata(&iso_path) {
                Ok(m) => m.len(),
                Err(e) => {
                    send(1.0, &format!("Error al leer ISO: {}", e), LogLevel::Error, true);
                    return;
                }
            };
            
            if let Ok(o) = Command::new("df").args(["--output=avail", "-B1", &dest_dir]).output() {
                let out = String::from_utf8_lossy(&o.stdout);
                if let Some(avail_str) = out.lines().nth(1) {
                    if let Ok(avail) = avail_str.trim().parse::<u64>() {
                        if avail < file_size {
                            send(1.0, "Cancelado: No hay espacio suficiente en el USB.", LogLevel::Error, true);
                            return;
                        }
                    }
                }
            }
            
            // 4. Copiar archivo
            let iso_name = std::path::Path::new(&iso_path).file_name().unwrap_or_default().to_string_lossy();
            let dest_file = format!("{}/{}", dest_dir, iso_name);
            
            send(0.15, &format!("Copiando {} ...", iso_name), LogLevel::Info, false);
            
            let src = match std::fs::File::open(&iso_path) {
                Ok(f) => f,
                Err(e) => { send(1.0, &format!("Error abriendo ISO: {}", e), LogLevel::Error, true); return; }
            };
            
            let dst = match std::fs::File::create(&dest_file) {
                Ok(f) => f,
                Err(e) => { send(1.0, &format!("Error creando archivo destino: {}", e), LogLevel::Error, true); return; }
            };
            
            use std::io::{BufReader, BufWriter, Read, Write};
            
            let mut reader = BufReader::with_capacity(1024 * 1024, src);
            let mut writer = BufWriter::with_capacity(1024 * 1024, dst);
            let mut copied = 0u64;
            let mut last_report = std::time::Instant::now();
            let mut last_copied = 0u64;
            let mut buffer = vec![0u8; 256 * 1024]; // 256KB buffer temperero para lectura
            
            loop {
                if cancelled() {
                    send(1.0, "Operacion cancelada", LogLevel::Warn, true);
                    let _ = std::fs::remove_file(&dest_file);
                    return;
                }
                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if let Err(e) = writer.write_all(&buffer[..n]) {
                            send(1.0, &format!("Error de escritura: {}", e), LogLevel::Error, true);
                            return;
                        }
                        copied += n as u64;
                    }
                    Err(e) => {
                        send(1.0, &format!("Error de lectura: {}", e), LogLevel::Error, true);
                        return;
                    }
                }
                
                let now = std::time::Instant::now();
                if now.duration_since(last_report).as_millis() > 500 {
                    let progress = 0.15 + (0.80 * (copied as f32 / file_size as f32));
                    let speed = (copied - last_copied) as f64 / now.duration_since(last_report).as_secs_f64();
                    send(progress, &format!("Copiando: {} / {} ({} / s)", 
                        format_size(copied), format_size(file_size), format_size(speed as u64)
                    ), LogLevel::Info, false);
                    last_report = now;
                    last_copied = copied;
                }
            }
            writer.flush().ok();
            
            // 5. Sync
            send(0.95, "Sincronizando disco (no extraiga el USB)...", LogLevel::Warn, false);
            let _ = Command::new("sync").args(["-f", &dest_dir]).output();
            
            send(1.0, "Flasheo completado correctamente", LogLevel::Ok, true);
        });
    }

    fn set_language(&mut self, lang: Language) {
        self.lang = lang;
        self.i18n = load_i18n(lang);
        save_app_config(lang, &self.config.download_dir, &self.config.speed_limit);
    }

    fn start_install_ventoy(&mut self, path: String, is_update: bool) {
        if self.op.active { return; }
        self.op = OpProgress::default();
        self.op.active = true;
        self.op.label  = if is_update { self.tf("ventoy_updating", &[&path]) } else { self.tf("ventoy_installing", &[&path]) };
        let act = if is_update { self.t("ventoy_updating_action") } else { self.t("ventoy_installing_action") };
        self.op.log(&format!("{} {} {}", act, if is_update {"update"} else {"install"}, path), LogLevel::Info);
        let (tx,rx)   = channel::<(f32,String,LogLevel,bool)>();
        let (ctx,crx) = channel::<()>();
        self.op.cancel_tx = Some(ctx);
        self.op_rx = Some(rx);
        std::thread::spawn(move || {
            let cancelled = || crx.try_recv().is_ok();
            let send = |p:f32, msg:&str, lvl:LogLevel, done:bool| { let _ = tx.send((p,msg.to_string(),lvl,done)); };
            // Verificar dispositivo
            send(0.05,&format!("Verificando dispositivo {}...",&path),LogLevel::Info,false);
            match Command::new("lsblk").args([&path]).output() {
                Err(e) => { send(0.0,&format!("Error: {}",e),LogLevel::Error,true); return; }
                Ok(o) if !o.status.success() => { send(0.0,&format!("Dispositivo {} no encontrado",&path),LogLevel::Error,true); return; }
                _ => {}
            }
            // Tamano real
            send(0.10,"Leyendo informacion del dispositivo...",LogLevel::Info,false);
            if let Ok(o) = Command::new("lsblk").args(["-b","-n","-o","SIZE",&path]).output() {
                let txt = String::from_utf8_lossy(&o.stdout);
                if let Ok(bytes) = txt.lines().next().unwrap_or("").trim().parse::<u64>() {
                    send(0.15,&format!("Tamano detectado: {:.1} GB", bytes as f64/1e9),LogLevel::Info,false);
                }
            }
            if cancelled() { send(0.0,"Cancelado",LogLevel::Warn,true); return; }
            // Buscar Ventoy2Disk.sh
            send(0.20,"Buscando Ventoy...",LogLevel::Info,false);
            let bin = find_ventoy_bin(&send);
            let bin = match bin {
                Some(b) => b,
                           None => {
                               send(0.28,"Ventoy no encontrado localmente. Descargando v1.1.12...",LogLevel::Warn,false);
                               let url     = "https://github.com/ventoy/Ventoy/releases/download/v1.1.12/ventoy-1.1.12-linux.tar.gz";
                               let tmp_gz  = "/tmp/ventoy-isoflash.tar.gz";
                               let tmp_dir = "/tmp/ventoy-isoflash";
                               let _ = std::fs::remove_file(tmp_gz);
                               let _ = std::fs::remove_dir_all(tmp_dir);
                               let dl_ok = Command::new("wget").args(["-q","-O",tmp_gz,url]).status().map(|s|s.success()).unwrap_or(false)
                               || Command::new("curl").args(["-L","-o",tmp_gz,url]).status().map(|s|s.success()).unwrap_or(false);
                               if !dl_ok { send(0.0,"Descarga fallida. Instala manualmente: paru -S ventoy",LogLevel::Error,true); return; }
                               if cancelled() { send(0.0,"Cancelado",LogLevel::Warn,true); return; }
                               send(0.45,"Extrayendo...",LogLevel::Info,false);
                               let _ = std::fs::create_dir_all(tmp_dir);
                               if let Ok(o) = Command::new("tar").args(["-xzf",tmp_gz,"-C",tmp_dir]).output() {
                                   if !o.status.success() { send(0.0,"Error extrayendo",LogLevel::Error,true); return; }
                               }
                               match Command::new("find").args([tmp_dir,"-name","Ventoy2Disk.sh","-maxdepth","3"])
                               .output().ok().and_then(|o| String::from_utf8_lossy(&o.stdout).lines().next().map(|l|l.trim().to_string()))
                               .filter(|s|!s.is_empty())
                               {
                                   Some(p) => { send(0.48,&format!("Script: {}",&p),LogLevel::Info,false); p }
                                   None    => { send(0.0,"No se encontro Ventoy2Disk.sh en el paquete",LogLevel::Error,true); return; }
                               }
                           }
            };
            if cancelled() { send(0.0,"Cancelado",LogLevel::Warn,true); return; }
            send(0.50,&format!("Usando: {}",&bin),LogLevel::Info,false);
            send(0.55,"Ejecutando instalacion — se pedira contrasena de administrador...",LogLevel::Warn,false);
            if bin.ends_with(".sh") { let _ = Command::new("chmod").args(["+x",&bin]).output(); }
            let flag = if is_update {"-u"} else {"-I"};
            // Ruta absoluta (pkexec/sudo reinician el CWD)
            let bin_abs = std::fs::canonicalize(&bin).unwrap_or_else(|_| std::path::PathBuf::from(&bin));
            let ventoy_dir = bin_abs.parent().and_then(|d| d.to_str()).unwrap_or(".").to_string();

            // Detectar el mejor programa askpass grafico disponible en el sistema
            let gui_askpass: Option<String> = [
                "/usr/bin/ksshaskpass",
                "/usr/lib/gcr4-ssh-askpass",
                "/usr/lib/gcr-ssh-askpass",
                "/usr/libexec/openssh/gnome-ssh-askpass",
                "/usr/lib/openssh/gnome-ssh-askpass",
                "/usr/lib/ssh/ssh-askpass",
                "/usr/lib/git-core/git-gui--askpass",
                "/usr/bin/ssh-askpass",
                "/usr/bin/x11-ssh-askpass",
            ].iter()
             .find(|p| std::path::Path::new(p).exists())
             .map(|p| p.to_string());

            if let Some(ref ap) = gui_askpass {
                send(0.56, &format!("Usando askpass GUI: {}", ap), LogLevel::Info, false);
            } else {
                send(0.56, "No se encontro un askpass GUI; se usara pkexec", LogLevel::Warn, false);
            }

            // Comando completo: Ventoy + partprobe/partx en el mismo bloque privilegiado
            // para que solo se pida la contrasena UNA sola vez via GUI.
            // Pasamos 'y' via printf para responder automaticamente a los prompts de confirmacion del script.
            // Las rutas se pasan via variables de entorno para evitar inyeccion de shell.
            let full_cmd = String::from(
                "cd \"$VENTOY_DIR\" && export PATH=\"$PWD/tool/x86_64:$PATH\" && printf 'y\\ny\\ny\\n' | \"$VENTOY_BIN\" $VENTOY_FLAG \"$VENTOY_DEV\" && partprobe \"$VENTOY_DEV\" ; partx -u \"$VENTOY_DEV\" ; udevadm settle --timeout=10"
            );

            // Variables de entorno para mostrar GUI en la sesion grafica actual
            let mut gui_env: Vec<(String,String)> = Vec::new();
            for v in &["DISPLAY","XAUTHORITY","WAYLAND_DISPLAY","DBUS_SESSION_BUS_ADDRESS","XDG_RUNTIME_DIR"] {
                if let Ok(val) = std::env::var(v) { gui_env.push((v.to_string(), val)); }
            }
            gui_env.push(("VENTOY_DIR".to_string(), ventoy_dir));
            gui_env.push(("VENTOY_BIN".to_string(), bin_abs.display().to_string()));
            gui_env.push(("VENTOY_FLAG".to_string(), flag.to_string()));
            gui_env.push(("VENTOY_DEV".to_string(), path.clone()));

            // ── Intento 1: sudo -A con askpass GUI ──────────────────────────────
            let result = if let Some(ref ap) = gui_askpass {
                let mut c = Command::new("sudo");
                c.args(["-A", "sh", "-c", &full_cmd]);
                c.env("SUDO_ASKPASS", ap);
                c.env("LANG","C").env("LC_ALL","C");
                for (k,v) in &gui_env { c.env(k,v); }
                let r = c.output();
                // Si sudo -A falla por contrasena incorrecta / cancelacion, no continuar
                if r.as_ref().map(|o|o.status.success()).unwrap_or(false) {
                    r
                } else {
                    // ── Intento 2: pkexec sh -c (muestra dialogo polkit nativo) ─
                    send(0.58,"Reintentando con pkexec...",LogLevel::Info,false);
                    let mut c2 = Command::new("pkexec");
                    c2.args(["sh", "-c", &full_cmd]);
                    for (k,v) in &gui_env { c2.env(k,v); }
                    c2.output()
                }
            } else {
                // No hay askpass GUI → usar pkexec directamente
                let mut c = Command::new("pkexec");
                c.args(["sh", "-c", &full_cmd]);
                for (k,v) in &gui_env { c.env(k,v); }
                c.output()
            };

            match result {
                Err(e) => { send(1.0,&format!("Error: {}",e),LogLevel::Error,true); }
                Ok(o)  => {
                    let stdout = String::from_utf8_lossy(&o.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&o.stderr).to_string();

                    if matches!(o.status.code(), Some(126) | Some(127)) {
                        send(1.0,"Cancelado: no se proporciono la contrasena de administrador.",LogLevel::Warn,true);
                        return;
                    }

                    if !o.status.success() {
                        for line in stderr.lines().take(6) {
                            let l = line.trim();
                            if !l.is_empty() { send(0.0,l,LogLevel::Error,false); }
                        }
                        for line in stdout.lines().take(6) {
                            let l = line.trim();
                            if !l.is_empty() { send(0.0,l,LogLevel::Error,false); }
                        }
                        send(1.0,"Instalacion fallida. Revisa los logs.",LogLevel::Error,true);
                        return;
                    }

                    // Script salio con exito. Partprobe/udevadm ya se ejecutaron dentro del bloque privilegiado.
                    // Hacer un udevadm settle extra como seguro antes de verificar.
                    send(0.80,"Verificando resultado de la instalacion...",LogLevel::Info,false);
                    let _ = Command::new("udevadm").args(["settle","--timeout=8"]).output();

                    // Verificacion real: comprobar si el dispositivo ahora tiene Ventoy.
                    // Las particiones pueden tardar varios segundos en aparecer tras un reparticionado.
                    let dev_name = path.trim_start_matches("/dev/").to_string();
                    let mut installed = false;
                    for attempt in 0..8 {
                        installed = check_ventoy(&dev_name);
                        if installed || is_update { break; }
                        if attempt < 7 {
                            send(0.85 + attempt as f32 * 0.01,
                                 &format!("Verificando dispositivo... ({}/7)", attempt+1),
                                 LogLevel::Info, false);
                            std::thread::sleep(Duration::from_secs(1));
                            // Solicitar al kernel que re-lea la tabla de particiones sin privilegios adicionales
                            if attempt == 2 {
                                let _ = Command::new("udevadm").args(["settle","--timeout=5"]).output();
                            }
                        }
                    }

                    if is_update || installed {
                        for line in stdout.lines().take(5) {
                            let l = line.trim();
                            if !l.is_empty() && !l.starts_with('*') { send(0.92,l,LogLevel::Info,false); }
                        }
                        std::thread::sleep(Duration::from_secs(1));
                        send(1.0,"Ventoy instalado correctamente!",LogLevel::Ok,true);
                    } else {
                        // El script termino bien pero la particion aun no es visible.
                        // Puede deberse a permisos de lectura de udev o un disco que requiere desconectar/reconectar.
                        send(0.0,"Ventoy2Disk.sh termino sin errores pero las particiones aun no son visibles.",LogLevel::Warn,false);
                        send(0.0,"Intenta desconectar y reconectar el USB — es posible que Ventoy si quedo instalado.",LogLevel::Warn,false);
                        send(1.0,"Instalacion posiblemente exitosa — reconecta el dispositivo para confirmar.",LogLevel::Warn,true);
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
                v.widgets.inactive.bg_fill  = Color32::from_rgb(30,30,42); v.widgets.inactive.rounding  = Rounding::same(8.0);
                v.widgets.hovered.bg_fill   = Color32::from_rgb(50,100,200); v.widgets.hovered.rounding  = Rounding::same(8.0);
                v.widgets.active.bg_fill    = Color32::from_rgb(40,80,180);  v.widgets.active.rounding   = Rounding::same(8.0);
                v.selection.bg_fill = Color32::from_rgb(40,80,180); v.override_text_color = None;
                ctx.set_visuals(v);
            }
            Tema::Claro => {
                let mut v = Visuals::light();
                v.panel_fill = Color32::from_rgb(245,246,250); v.window_fill = Color32::WHITE;
                v.extreme_bg_color = Color32::from_rgb(230,232,240);
                v.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(50,55,80);
                v.widgets.noninteractive.bg_fill = Color32::from_rgb(245,246,250);
                v.widgets.inactive.bg_fill  = Color32::from_rgb(225,227,240); v.widgets.inactive.rounding  = Rounding::same(8.0);
                v.widgets.inactive.fg_stroke.color = Color32::from_rgb(55,60,90);
                v.widgets.hovered.bg_fill   = Color32::from_rgb(100,140,230); v.widgets.hovered.rounding  = Rounding::same(8.0);
                v.widgets.hovered.fg_stroke.color  = Color32::WHITE;
                v.widgets.active.bg_fill    = Color32::from_rgb(70,110,210);  v.widgets.active.rounding   = Rounding::same(8.0);
                v.widgets.active.fg_stroke.color   = Color32::WHITE;
                v.selection.bg_fill = Color32::from_rgb(70,110,210);
                v.override_text_color = None; // dejar que cada widget maneje su propio color
                ctx.set_visuals(v);
            }
        }
    }
}

impl eframe::App for IsoFlash {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        save_dl_state(&self.downloads);
        save_app_config(self.lang, &self.config.download_dir, &self.config.speed_limit);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i|i.time);

        // ── Auto-scan USB cada 2s (silencioso) ──
        if !self.scanning && (now-self.last_scan)>=2.0 {
            self.last_scan = now; self.scanning = true;
            let (tx,rx) = channel();
            self.usb_rx = Some(rx);
            std::thread::spawn(move || { let _ = tx.send(scan_usbs()); });
        }
        if let Some(rx) = &self.usb_rx {
            if let Ok(usbs) = rx.try_recv() { self.usbs=usbs; self.scanning=false; self.usb_rx=None; }
        }

        // ── Rescan forzado post-Ventoy ──
        if let Some(at) = self.rescan_after {
            if now>=at { self.rescan_after=None; self.last_scan=-999.0; }
        }

        // ── Red: check cada 30s ──
        if (now-self.last_net_check)>=30.0 && self.network_rx.is_none() {
            self.last_net_check = now;
            let (tx,rx) = channel();
            self.network_rx = Some(rx);
            std::thread::spawn(move || { let _ = tx.send(check_network()); });
        }
        if let Some(rx) = &self.network_rx {
            if let Ok(ok) = rx.try_recv() { self.has_network=ok; self.network_rx=None; }
        }

        // ── Catalogo remoto: descargar si hay red (cada 5 min) ──
        if self.has_network && !self.catalog_updating && self.catalog_rx.is_none() && (now - self.last_catalog_update) > 300.0 {
            self.catalog_updating = true;
            let (tx,rx) = channel();
            self.catalog_rx = Some(rx);
            std::thread::spawn(move || { let _ = tx.send(fetch_remote_catalog()); });
        }
        if let Some(rx) = &self.catalog_rx {
            if let Ok(result) = rx.try_recv() {
                if let Some(json) = result {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
                        if let Some(ver) = v["version"].as_str() { self.catalog_version = ver.to_string(); }
                        if let Some(upd) = v["updated"].as_str() { self.catalog_updated = upd.to_string(); }
                    }
                    let remote_hash = hash_str(&json);
                    if remote_hash != self.catalog_hash {
                        let new_catalog = load_catalog(&json);
                        if !new_catalog.is_empty() {
                            self.catalog = new_catalog;
                            self.catalog_hash = remote_hash;
                            self.notif = Some((self.t("notif_catalog_updated").to_string(), now));
                            // Trigger URL resolution on fresh catalog
                            self.catalog_resolve_requested = true;
                        }
                    }
                }
                self.catalog_rx = None; self.catalog_updating = false; self.last_catalog_update = now;
            }
        }

        // ── Resolver URLs del catálogo ──
        if self.catalog_resolve_requested && self.catalog_resolve_rx.is_none() && !self.catalog.is_empty() && self.has_network {
            self.catalog_resolve_requested = false;
            let catalog = self.catalog.clone();
            let (tx, rx) = channel();
            self.catalog_resolve_rx = Some(rx);
            std::thread::spawn(move || {
                let mut resolved = catalog;
                for distro in &mut resolved {
                    if distro.is_windows { continue; }
                    if let Some(new_url) = resolver::resolve_url(&distro.name, &distro.url) {
                        distro.url = new_url;
                    }
                }
                let _ = tx.send(Some(resolved));
            });
        }
        if let Some(rx) = &self.catalog_resolve_rx {
            if let Ok(Some(new_catalog)) = rx.try_recv() {
                let changed = new_catalog.iter().zip(self.catalog.iter()).filter(|(n, o)| n.url != o.url).count();
                self.catalog = new_catalog;
                self.catalog_resolve_rx = None;
                if changed > 0 {
                    self.notif = Some((format!("🔗 {} URLs actualizadas", changed), now));
                }
            } else if let Ok(None) = rx.try_recv() {
                self.catalog_resolve_rx = None;
            }
        }

        // ── Dismiss notificacion despues de 4s ──
        if let Some((_, start)) = self.notif {
            if now - start > 4.0 { self.notif = None; }
            else { ctx.request_repaint(); }
        }

        // ── Progreso Ventoy ──
        let mut ventoy_ok = false;
        if let Some(rx) = &self.op_rx {
            while let Ok((progress,msg,level,done)) = rx.try_recv() {
                if progress>0.0 { self.op.progress=progress; }
                if done && level==LogLevel::Ok { ventoy_ok=true; }
                self.op.log(&msg,level);
                if done { self.op.active=false; self.op.cancel_tx=None; self.op_rx=None; break; }
            }
        }
        if ventoy_ok { self.rescan_after=Some(now+1.5); }

        // ── Progreso descargas ──
        for dl in &mut self.downloads {
            if let Some(rx) = &dl.progress_rx {
                while let Ok(p) = rx.try_recv() {
                    if let Some(e) = &p.error {
                        if e == "__PAUSED__" { dl.status=DownloadStatus::Paused; }
                        else { dl.status=DownloadStatus::Error(e.clone()); }
                        dl.progress_rx=None; dl.pause_tx=None; break;
                    } else if p.done {
                        dl.status=DownloadStatus::Done; dl.progress=1.0;
                        dl.progress_rx=None; dl.pause_tx=None; break;
                    } else {
                        dl.progress=p.progress; dl.speed_str=p.speed;
                    }
                }
            }
        }

        // ── Autocompletado de ruta ──
        if self.panel==Panel::Configuracion {
            self.path_sug = path_suggestions(&self.config.download_dir);
        }

        // ── Animacion tema suave ──
        let t_target = match self.tema { Tema::Oscuro=>0.0_f32, Tema::Claro=>1.0_f32 };
        self.tema_anim += (t_target-self.tema_anim)*0.10;
        if (self.tema_anim-t_target).abs()>0.002 { ctx.request_repaint(); }
        let panel_now   = lerp_color(Color32::from_rgb(15,15,20),  Color32::from_rgb(245,246,250), self.tema_anim);
        let sidebar_now = lerp_color(Color32::from_rgb(18,18,26),  Color32::from_rgb(235,237,245), self.tema_anim);
        { let mut v=ctx.style().visuals.clone(); v.panel_fill=panel_now; ctx.set_visuals(v); }

        // Título de ventana fijo
        ctx.send_viewport_cmd(egui::ViewportCommand::Title("IsoFlash".to_string()));

        // Logo pulso — usa t del frame actual
        let t   = ctx.input(|i|i.time) as f32;
        let pl  = ((t*1.8).sin()*0.12+0.88).clamp(0.0,1.0);
        let logo_col = Color32::from_rgb((80.0*pl) as u8,(140.0*pl) as u8,(255.0*pl) as u8);

        // Repaint continuo solo cuando hay animaciones activas
        let needs_continuous = self.op.active
        || self.catalog_updating
        || self.downloads.iter().any(|d| d.status == DownloadStatus::Downloading)
        || (self.tema_anim - t_target).abs() > 0.01;
        if needs_continuous {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }

        let th  = self.tema.colors();
        let op_active = self.op.active;
        let op_cancel = self.op.cancel_tx.is_some();
        let has_dl    = !self.downloads.is_empty();

        // ── Sidebar ──
        egui::SidePanel::left("sidebar").exact_width(170.0)
        .frame(Frame::none().fill(sidebar_now).inner_margin(10.0))
        .show(ctx, |ui| {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚡ IsoFlash").size(20.0).strong().color(logo_col));
                if !self.has_network {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("●").size(10.0).color(Color32::from_rgb(180,80,80)))
                    .on_hover_text(self.t("offline_indicator"));
                }
            });
            // Notificacion temporal
            if let Some((ref msg, _)) = self.notif {
                ui.add_space(2.0);
                let nc = Color32::from_rgb(80,200,120);
                ui.label(egui::RichText::new(msg.as_str()).size(11.0).color(nc));
            }
            ui.add_space(24.0);
            let nav_dashboard = self.t("nav_dashboard").to_string();
            let nav_catalog = self.t("nav_catalog").to_string();
            let nav_downloads = self.t("nav_downloads").to_string();
            let nav_local_isos = self.t("nav_local_isos").to_string();
            let nav_flash = self.t("nav_flash").to_string();
            let nav_persistence = self.t("nav_persistence").to_string();
            let nav_logs = self.t("nav_logs").to_string();
            let nav_configuration = self.t("nav_configuration").to_string();
            let theme_light = self.t("theme_light").to_string();
            let theme_dark = self.t("theme_dark").to_string();
            sidebar_btn(ui,ctx,&mut self.panel,&th,Panel::Dashboard,    "🖥",&nav_dashboard, false); ui.add_space(4.0);
            sidebar_btn(ui,ctx,&mut self.panel,&th,Panel::Catalogo,     "📦",&nav_catalog, false); ui.add_space(4.0);
            sidebar_btn(ui,ctx,&mut self.panel,&th,Panel::Descargas,    "⬇",&nav_downloads, has_dl); ui.add_space(4.0);
            sidebar_btn(ui,ctx,&mut self.panel,&th,Panel::Locales,      "💾",&nav_local_isos, false); ui.add_space(4.0);
            sidebar_btn(ui,ctx,&mut self.panel,&th,Panel::Flasheo,      "🔥",&nav_flash, false); ui.add_space(4.0);
            sidebar_btn(ui,ctx,&mut self.panel,&th,Panel::Persistencia, "💿",&nav_persistence, false); ui.add_space(4.0);
            sidebar_btn(ui,ctx,&mut self.panel,&th,Panel::Logs,         "📋",&nav_logs, op_active);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.add_space(8.0);
                let (ico,lbl): (&str, &str) = match self.tema { Tema::Oscuro=>("\u{2600}",&theme_light), Tema::Claro=>("\u{1F319}",&theme_dark) };
                let tfc = th.label_dim;
                if ui.add(egui::Button::new(egui::RichText::new(format!("{ico}  {lbl}")).size(13.0).color(tfc))
                    .fill(sidebar_now).rounding(Rounding::same(8.0)).min_size(Vec2::new(150.0,34.0))).clicked() {
                        self.tema = match self.tema { Tema::Oscuro=>Tema::Claro, Tema::Claro=>Tema::Oscuro };
                        self.apply_theme(ctx);
                    }
                    ui.add_space(4.0);
                    sidebar_btn(ui,ctx,&mut self.panel,&th,Panel::Configuracion,"⚙",&nav_configuration,false);
            });
        });

        let mut dash_action: Option<DashAction> = None;

        egui::CentralPanel::default()
        .frame(Frame::none().fill(panel_now).inner_margin(egui::Margin{left:20.0,right:20.0,top:0.0,bottom:0.0}))
        .show(ctx, |ui| {
            ui.add_space(20.0);
            // Header
            ui.horizontal(|ui| {
                let (tit,sub) = match self.panel {
                    Panel::Dashboard     => (self.t("nav_dashboard"),     self.t("panel_dashboard_sub")),
                          Panel::Catalogo      => (self.t("nav_catalog"),       self.t("panel_catalog_sub")),
                          Panel::Descargas     => (self.t("nav_downloads"),     self.t("panel_downloads_sub")),
                          Panel::Locales       => (self.t("nav_local_isos"),    self.t("panel_local_sub")),
                          Panel::Flasheo       => (self.t("nav_flash"),         self.t("panel_flash_sub")),
                          Panel::Persistencia  => (self.t("nav_persistence"),   self.t("panel_persistence_sub")),
                          Panel::Logs          => (self.t("nav_logs"),          self.t("panel_logs_sub")),
                          Panel::Configuracion => (self.t("nav_configuration"), self.t("panel_configuration_sub")),
                };
                let fade = ctx.animate_value_with_time(egui::Id::new("panel_fade"), 1.0, 0.25);
                let a = (fade*255.0) as u8;
                let sc = Color32::from_rgba_premultiplied(th.text.r(), th.text.g(), th.text.b(), a);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(tit).size(26.0).strong().color(Color32::from_rgba_premultiplied(60,120,240,a)));
                    ui.label(egui::RichText::new(sub).size(13.0).color(sc));
                });
            });
            ui.add_space(10.0); ui.separator(); ui.add_space(12.0);

            let mut lang_changed = false;
            let mut go_downloads = false;

            match self.panel {
                Panel::Dashboard => draw_dashboard(ui, &self.usbs, self.scanning, op_active, op_cancel, &th, &self.i18n, &mut dash_action),
              Panel::Catalogo  => {
                  let mut resolve_trigger = false;
                  draw_catalog(ui, &self.catalog, &mut self.cat_search, &mut self.cat_filter, &mut self.cat_win_popup, &mut self.cat_win_name, &mut self.downloads, &self.config, &th, &self.i18n, self.catalog_updating, self.catalog_resolve_rx.is_some(), &self.catalog_version, &self.catalog_updated, &mut resolve_trigger, &mut go_downloads);
                  if resolve_trigger { self.catalog_resolve_requested = true; }
              }
              Panel::Descargas => {
                  if let Some(act) = draw_descargas(ui, &mut self.downloads, &self.config, &th, &self.i18n) {
                      match act {
                          DlAction::Start(i)   => start_download(&mut self.downloads[i], &self.config),
              DlAction::Pause(i)   => { if let Some(tx) = self.downloads[i].pause_tx.take() { let _ = tx.send(()); } }
              DlAction::Remove(i)  => { self.downloads.remove(i); }
              DlAction::OpenDir(i) => {
                  if let Some(p) = std::path::Path::new(&self.downloads[i].dest_path).parent() {
                      let _ = Command::new("xdg-open").arg(p).spawn();
                  }
              }
              DlAction::ClearDone  => self.downloads.retain(|d|d.status!=DownloadStatus::Done),
                      }
                  }
              }
              Panel::Locales => {
                  if draw_locales(ui, &self.iso_files, &self.config.download_dir, &th, &self.i18n) {
                      self.iso_files = scan_iso_files(&self.config.download_dir);
                  }
              }
              Panel::Configuracion => draw_configuracion(ui, &mut self.config, &self.path_sug.clone(), &mut self.show_path_sug, &th, &self.i18n, &mut self.lang, &mut lang_changed),
              Panel::Flasheo => {
                  if let Some((u, i)) = draw_flasheo(ui, &self.usbs, &self.iso_files, &self.downloads, &mut self.flash_target_usb, &mut self.flash_target_iso, &th, &self.i18n, op_active) {
                      self.start_flash_iso(u, i);
                  }
              }
              Panel::Logs => draw_logs(ui, ctx, &mut self.op, &th, &self.i18n),
              _ => {
                  ui.vertical_centered(|ui| { ui.add_space(80.0); ui.label(egui::RichText::new(format!("\u{1F6A7}  {}", self.t("logs_under_construction"))).size(16.0).color(th.text)); });
              }
            }

            if lang_changed {
                self.set_language(self.lang);
            }

            if go_downloads { self.panel = Panel::Descargas; }
        });

        // ── Popup Windows ──
        if self.cat_win_popup {
            let url = if self.cat_win_name.contains("11") { "https://www.microsoft.com/software-download/windows11" } else { "https://www.microsoft.com/software-download/windows10" };
            let title = self.tf("windows_popup_title", &[&self.cat_win_name]);
            egui::Window::new(format!("\u{1FA9F}  {}", title))
            .collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).fixed_size([440.0,0.0])
            .show(ctx, |ui| {
                ui.add_space(6.0);
                Frame::none().fill(Color32::from_rgb(60,40,10)).rounding(Rounding::same(8.0)).inner_margin(12.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(format!("\u{26A0}  {}", self.t("windows_popup_warning"))).size(13.0).strong().color(Color32::from_rgb(230,170,60)));
                });
                ui.add_space(10.0);
                ui.label(self.t("windows_popup_desc"));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(self.t("windows_popup_steps")).size(13.0).strong());
                ui.label(self.t("windows_popup_step1"));
                ui.label(self.t("windows_popup_step2"));
                ui.label(self.t("windows_popup_step3"));
                ui.label(self.t("windows_popup_step4"));
                ui.add_space(10.0);
                Frame::none().fill(th.panel_fill).rounding(Rounding::same(6.0)).inner_margin(8.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(url).size(11.0).monospace().color(Color32::from_rgb(80,160,240)));
                });
                ui.add_space(12.0);
                if ui.add(egui::Button::new(egui::RichText::new(self.t("windows_popup_close")).size(13.0).color(Color32::WHITE)).fill(Color32::from_rgb(40,80,180)).rounding(Rounding::same(7.0)).min_size(Vec2::new(100.0,30.0))).clicked() {
                    self.cat_win_popup = false;
                }
                ui.add_space(4.0);
            });
        }

        // ── Acciones Dashboard ──
        if let Some(action) = dash_action {
            match action {
                DashAction::InstallVentoy(path,is_upd) => self.start_install_ventoy(path,is_upd),
                DashAction::CancelVentoy => {
                    if let Some(tx) = self.op.cancel_tx.take() { let _ = tx.send(()); }
                    self.op.active = false;
                    self.op.log("Operacion cancelada por el usuario", LogLevel::Warn);
                }
                DashAction::GoFlash(path) => {
                    self.flash_target_usb = Some(path);
                    self.panel = Panel::Flasheo;
                }
            }
        }
    }
}
