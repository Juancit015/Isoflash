use std::process::Command;

fn http_get(url: &str) -> Option<String> {
    let result = Command::new("curl")
        .args(["-sL", "--max-time", "8", url])
        .output();
    if let Ok(o) = &result {
        if o.status.success() {
            return Some(String::from_utf8_lossy(&o.stdout).to_string());
        }
    }
    Command::new("wget")
        .args(["-qO", "-", "--timeout=8", url])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

fn extract_hrefs(html: &str) -> Vec<String> {
    let mut hrefs = Vec::new();
    let mut pos = 0;
    while let Some(s) = html[pos..].find("href=\"") {
        let start = pos + s + 6;
        if let Some(end) = html[start..].find('\"') {
            let h = html[start..start + end].to_string();
            if !h.starts_with('?') && !h.starts_with('/') && !h.starts_with("http") && !h.is_empty() {
                hrefs.push(h.trim_end_matches('/').to_string());
            }
            pos = start + end + 1;
        } else { break; }
    }
    hrefs
}

fn parse_ver(s: &str) -> Option<(u64, u64, u64)> {
    let p: Vec<&str> = s.split('.').collect();
    if p.len() >= 2 {
        Some((p[0].parse().ok()?, p[1].parse().ok()?, p.get(2).and_then(|x| x.parse().ok()).unwrap_or(0)))
    } else { None }
}

fn latest_dir(hrefs: &[String]) -> Option<String> {
    let mut v: Vec<(u64, u64, u64, String)> = hrefs.iter().filter_map(|h| {
        parse_ver(h).map(|(a, b, c)| (a, b, c, h.clone()))
    }).collect();
    v.sort();
    v.last().map(|(_, _, _, s)| s.clone())
}

fn arch() -> Option<String> {
    let body = http_get("https://archlinux.org/releng/releases/json/")?;
    let j: serde_json::Value = serde_json::from_str(&body).ok()?;
    j["iso_url"].as_str().map(|s| s.to_string())
}

fn ubuntu_flavor(flavor: &str) -> Option<String> {
    let base = if flavor.is_empty() { "https://releases.ubuntu.com/".to_string() }
               else { format!("https://cdimage.ubuntu.com/{}/releases/", flavor) };
    let body = http_get(&base)?;
    let version = latest_dir(&extract_hrefs(&body))?;
    let name = if flavor.is_empty() { "ubuntu" } else { flavor };
    if flavor.is_empty() {
        Some(format!("https://releases.ubuntu.com/{}/ubuntu-{}-desktop-amd64.iso", version, version))
    } else {
        Some(format!("https://cdimage.ubuntu.com/{}/releases/{}/release/{}-{}-desktop-amd64.iso", flavor, version, name, version))
    }
}

fn fedora_ver() -> Option<String> {
    let body = http_get("https://dl.fedoraproject.org/pub/fedora/linux/releases/")?;
    latest_dir(&extract_hrefs(&body))
}

fn debian_netinst() -> Option<String> {
    let body = http_get("https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/")?;
    let iso = extract_hrefs(&body).iter().find(|h| h.ends_with("-amd64-netinst.iso"))?.clone();
    Some(format!("https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/{}", iso))
}

fn mint_version() -> Option<String> {
    let body = http_get("https://mirrors.layeronline.com/linuxmint/stable/")?;
    latest_dir(&extract_hrefs(&body))
}

fn alpine_latest(major_minor: &str) -> Option<String> {
    let url = format!("https://dl-cdn.alpinelinux.org/alpine/v{}/releases/x86_64/", major_minor);
    let body = http_get(&url)?;
    let iso = extract_hrefs(&body).iter().find(|h| h.starts_with("alpine-standard") && h.ends_with("-x86_64.iso"))?.clone();
    Some(format!("https://dl-cdn.alpinelinux.org/alpine/v{}/releases/x86_64/{}", major_minor, iso))
}

type Resolver = fn(name: &str, old_url: &str) -> Option<String>;

struct Entry {
    patterns: &'static [&'static str],
    func: Resolver,
}

static RESOLVERS: &[Entry] = &[
    Entry { patterns: &["Arch Linux"], func: |_, _| arch() },

    Entry { patterns: &["Ubuntu "], func: |_, _| ubuntu_flavor("") },
    Entry { patterns: &["Kubuntu"], func: |_, _| ubuntu_flavor("kubuntu") },
    Entry { patterns: &["Xubuntu"], func: |_, _| ubuntu_flavor("xubuntu") },
    Entry { patterns: &["Lubuntu"], func: |_, _| ubuntu_flavor("lubuntu") },
    Entry { patterns: &["Ubuntu Budgie"], func: |_, _| ubuntu_flavor("ubuntu-budgie") },
    Entry { patterns: &["Ubuntu Cinnamon"], func: |_, _| ubuntu_flavor("ubuntucinnamon") },
    Entry { patterns: &["Ubuntu Studio"], func: |_, _| ubuntu_flavor("ubuntustudio") },

    Entry { patterns: &["Ubuntu Server"], func: |_, _| ubuntu_flavor("ubuntu") },

    Entry { patterns: &["Linux Mint"], func: |n, _| {
        let ed = if n.contains("Xfce") { "xfce" } else if n.contains("MATE") { "mate" } else { "cinnamon" };
        mint_version().map(|v| format!("https://mirrors.layeronline.com/linuxmint/stable/{}/linuxmint-{}-{}-64bit.iso", v, v, ed))
    }},
    Entry { patterns: &["LMDE"], func: |_, _| {
        mint_version().map(|v| format!("https://mirrors.layeronline.com/linuxmint/debian/{}/lmde-{}-cinnamon-64bit.iso", v, v))
    }},

    Entry { patterns: &["Fedora 44 Workstation"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Workstation/x86_64/iso/Fedora-Workstation-Live-x86_64-{}-1.1.iso", v, v))
    }},
    Entry { patterns: &["Fedora 44 KDE"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/x86_64/iso/Fedora-KDE-Live-x86_64-{}-1.1.iso", v, v))
    }},
    Entry { patterns: &["Fedora 44 Xfce"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/x86_64/iso/Fedora-Xfce-Live-x86_64-{}-1.1.iso", v, v))
    }},
    Entry { patterns: &["Fedora 44 Cinnamon"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/x86_64/iso/Fedora-Cinnamon-Live-x86_64-{}-1.1.iso", v, v))
    }},
    Entry { patterns: &["Fedora 44 MATE-Compiz"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/x86_64/iso/Fedora-MATE_Compiz-Live-x86_64-{}-1.1.iso", v, v))
    }},
    Entry { patterns: &["Fedora 44 i3"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/x86_64/iso/Fedora-i3-Live-x86_64-{}-1.1.iso", v, v))
    }},
    Entry { patterns: &["Fedora 44 LXQt"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/x86_64/iso/Fedora-LXQt-Live-x86_64-{}-1.1.iso", v, v))
    }},
    Entry { patterns: &["Fedora 44 Sway"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Spins/x86_64/iso/Fedora-Sway-Live-x86_64-{}-1.1.iso", v, v))
    }},
    Entry { patterns: &["Fedora 44 Server"], func: |_, _| {
        fedora_ver().map(|v| format!("https://dl.fedoraproject.org/pub/fedora/linux/releases/{}/Server/x86_64/iso/Fedora-Server-dvd-x86_64-{}-1.1.iso", v, v))
    }},

    Entry { patterns: &["Debian"], func: |n, _| {
        if n.contains("Netinstall") || n.contains("netinst") {
            debian_netinst()
        } else {
            None
        }
    }},

    Entry { patterns: &["Alpine"], func: |_, old| {
        let version = old.split('/').nth(3)?.trim_start_matches('v');
        alpine_latest(version)
    }},

    Entry { patterns: &["NixOS"], func: |_, old| {
        let body = http_get("https://channels.nixos.org/")?;
        let channels = extract_hrefs(&body);
        let nixos: Vec<&str> = channels.iter().filter(|c| c.starts_with("nixos-")).map(|s| s.as_str()).collect();
        if nixos.is_empty() { return None; }
        let latest = nixos.last()?;
        let edition = if old.contains("gnome") { "gnome" } else if old.contains("plasma6") || old.contains("kde") { "plasma6" } else { "minimal" };
        Some(format!("https://channels.nixos.org/{}/latest-nixos-{}-x86_64-linux.iso", latest, edition))
    }},

    Entry { patterns: &["openSUSE Tumbleweed"], func: |_, _| {
        Some("https://download.opensuse.org/tumbleweed/iso/openSUSE-Tumbleweed-DVD-x86_64-Current.iso".to_string())
    }},

    Entry { patterns: &["Void Linux"], func: |_, old| {
        let parts: Vec<&str> = old.rsplit('/').collect();
        let filename = parts.first()?;
        let base = old.trim_end_matches(filename);
        Some(format!("{}void-live-x86_64-{}.iso", base, {
            let body = http_get(&base)?;
            let files = extract_hrefs(&body);
            files.iter().find(|f| f.starts_with("void-live-x86_64") && f.ends_with("-xfce.iso"))?.trim_end_matches("-xfce.iso").to_string()
        }))
    }},
    Entry { patterns: &["Void Linux musl"], func: |_, old| {
        let parts: Vec<&str> = old.rsplit('/').collect();
        let filename = parts.first()?;
        let base = old.trim_end_matches(filename);
        Some(format!("{}void-live-x86_64-musl-{}.iso", base, {
            let body = http_get(&base)?;
            let files = extract_hrefs(&body);
            files.iter().find(|f| f.contains("musl") && f.ends_with(".iso"))?.trim_end_matches(".iso").to_string()
        }))
    }},
    Entry { patterns: &["Void Linux base"], func: |_, old| {
        let parts: Vec<&str> = old.rsplit('/').collect();
        let filename = parts.first()?;
        let base = old.trim_end_matches(filename);
        Some(format!("{}void-live-x86_64-{}.iso", base, {
            let body = http_get(&base)?;
            let files = extract_hrefs(&body);
            files.iter().find(|f| f.starts_with("void-live-x86_64-2025") && f.ends_with(".iso") && !f.contains("xfce") && !f.contains("musl"))?.trim_end_matches(".iso").to_string()
        }))
    }},

    Entry { patterns: &["KDE neon"], func: |_, _| {
        let body = http_get("https://files.kde.org/neon/images/user/")?;
        let dirs = extract_hrefs(&body);
        let latest = dirs.iter().filter(|d| d.len() == 8 && d.chars().all(|c| c.is_ascii_digit())).max()?;
        Some(format!("https://files.kde.org/neon/images/user/{}/neon-user-{}.iso", latest, latest))
    }},

    Entry { patterns: &["Deepin"], func: |_, _| {
        let body = http_get("https://cdimage.deepin.com/releases/")?;
        let version = latest_dir(&extract_hrefs(&body))?;
        Some(format!("https://cdimage.deepin.com/releases/{}/deepin-desktop-community-{}-amd64.iso", version, version))
    }},

    Entry { patterns: &["openSUSE Leap"], func: |_, old| {
        let parts: Vec<&str> = old.split('/').collect();
        if parts.len() >= 7 {
            let base = format!("https://download.opensuse.org/distribution/leap/{}/iso/", parts[4]);
            let body = http_get(&base)?;
            let iso = extract_hrefs(&body).iter().find(|h| h.ends_with("-DVD-x86_64-Media.iso"))?.clone();
            Some(format!("{}{}", base, iso))
        } else { None }
    }},
];

pub fn resolve_url(name: &str, old_url: &str) -> Option<String> {
    for entry in RESOLVERS {
        if entry.patterns.iter().any(|p| name.contains(p)) {
            if let Some(url) = (entry.func)(name, old_url) {
                if !url.is_empty() {
                    return Some(url);
                }
            }
        }
    }
    None
}
