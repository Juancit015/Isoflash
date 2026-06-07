# ⚡ IsoFlash

IsoFlash es una aplicación de escritorio escrita en Rust que simplifica la creación de medios booteables USB para Linux y Windows.

Combina detección automática de dispositivos, instalación de Ventoy, catálogo integrado de distribuciones y herramientas de gestión en una única interfaz moderna.

> Estado del proyecto: Alpha temprana 🚧

---

## ✨ Características

### 🔌 Gestión de USB

- Detección automática de dispositivos USB
- Escaneo en tiempo real
- Identificación de unidades con Ventoy instalado
- Instalación y actualización de Ventoy
- Cancelación segura de operaciones

### 📦 Catálogo Integrado

Incluye acceso rápido a distribuciones populares:

- Arch Linux
- EndeavourOS
- Manjaro
- Fedora
- Debian
- Ubuntu
- Linux Mint
- openSUSE
- Alpine Linux
- Kali Linux
- Parrot OS
- Tails
- Nobara
- CachyOS
- Bazzite
- Windows 10 y 11

Con filtros por categorías:

- Rolling Release
- LTS
- Servidor
- Seguridad
- Gaming
- Windows

### ⬇ Sistema de Descargas

- Cola de descargas
- Gestión de múltiples ISOs
- Seguimiento de progreso
- Acceso rápido a fuentes oficiales

### 📋 Registro de Operaciones

- Logs detallados
- Seguimiento de instalación
- Información de errores
- Historial de acciones

### 🎨 Interfaz Moderna

- Modo oscuro
- Modo claro
- Animaciones suaves
- Diseño responsivo
- Construido con egui

---

## 🛠 Tecnologías

- Rust
- egui
- eframe
- serde_json
- lsblk
- Ventoy

---

## 🚀 Compilación

### Requisitos

- Rust estable
- Cargo
- Linux
- Ventoy (opcional)

### Clonar

```bash
git clone https://github.com/TU-USUARIO/isoflash.git
cd isoflash
```

### Ejecutar

```bash
cargo run
```

### Compilar versión optimizada

```bash
cargo build --release
```

El binario se generará en:

```text
target/release/isoflash
```

---

## 📌 Hoja de Ruta

### Alpha

- [x] Detección de USB
- [x] Integración con Ventoy
- [x] Catálogo de distribuciones
- [x] Sistema de logs
- [x] Cola de descargas

### Beta

- [ ] Descargas reales integradas
- [ ] Verificación SHA256
- [ ] Persistencia para distribuciones Live
- [ ] Gestión de ISOs locales
- [ ] Flasheo directo de imágenes

### Futuro

- [ ] Soporte Windows
- [ ] Soporte macOS
- [ ] Multilenguaje
- [ ] Actualizaciones automáticas
- [ ] Tienda de imágenes personalizadas

---

## ⚠ Advertencia

IsoFlash puede modificar tablas de particiones y sobrescribir completamente dispositivos USB.

Verifica siempre el dispositivo seleccionado antes de ejecutar operaciones de escritura.

---

## 📄 Licencia

Actualmente en desarrollo.

La licencia será definida antes de la primera versión estable.
