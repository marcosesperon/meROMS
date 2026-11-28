//! Sets de arcade: ¿con qué emulador funciona cada zip? Cada núcleo (FinalBurn Neo, MAME 2003-Plus,
//! MAME 2010, MAME actual) espera los chips de *su* versión de MAME, así que se compara el índice
//! del zip (el CRC de cada chip, sin descomprimir) con el DAT de cada núcleo.
//!
//! Aquí va lo que no toca la app: leer un DAT a un índice compacto (`parse_index`) y
//! diagnosticar un set con ese índice (`diagnose`). La descarga y los comandos están en
//! `commands/arcade.rs`.

use std::collections::{HashMap, HashSet};
use std::io::BufRead;

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};

/// Un chip del set
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chip {
    /// Nombre (solo para enseñar cuáles faltan)
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "c")]
    pub crc: u32,
    /// Viene del padre o de la BIOS (`merge`): en un set «split» no está en el zip del juego
    #[serde(rename = "m", default, skip_serializing_if = "std::ops::Not::not")]
    pub merged: bool,
    /// De una BIOS que no es la predeterminada (`bios="…"`): no hace falta
    #[serde(rename = "o", default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
}

/// Una máquina (juego, BIOS o dispositivo) del DAT, solo con lo que hace falta para comprobar
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    #[serde(rename = "co", default, skip_serializing_if = "Option::is_none")]
    pub cloneof: Option<String>,
    #[serde(rename = "ro", default, skip_serializing_if = "Option::is_none")]
    pub romof: Option<String>,
    #[serde(rename = "b", default, skip_serializing_if = "std::ops::Not::not")]
    pub bios: bool,
    #[serde(rename = "dv", default, skip_serializing_if = "std::ops::Not::not")]
    pub device: bool,
    /// Estado del driver: 0 bueno, 1 imperfecto, 2 preliminar, 3 no lo dice
    #[serde(rename = "s", default = "unknown_status")]
    pub status: u8,
    #[serde(rename = "r", default, skip_serializing_if = "Vec::is_empty")]
    pub chips: Vec<Chip>,
    /// Discos CHD que necesita (nombre sin extensión)
    #[serde(rename = "d", default, skip_serializing_if = "Vec::is_empty")]
    pub disks: Vec<String>,
    /// Dispositivos con ROMs propias que usa (MAME actual: `qsound`…)
    #[serde(rename = "dr", default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<String>,
}

fn unknown_status() -> u8 {
    3
}

/// El DAT de un núcleo, resumido
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArcadeIndex {
    /// Versión que dice el DAT («0.289», «1.0.0.03», «0.139»…)
    pub version: String,
    pub machines: HashMap<String, Machine>,
}

// ---------- Lectura del DAT ----------

/// Lee un DAT de arcade: MAME `-listxml` (cualquier versión, `<game>` o `<machine>`) o
/// Logiqx/clrmamepro (FBNeo). En streaming: el de MAME actual pasa de 250 MB.
pub fn parse_index<R: BufRead>(input: R) -> Result<ArcadeIndex, String> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut index = ArcadeIndex::default();
    // Lo que se está leyendo: la máquina y su BIOS predeterminada
    let mut current: Option<(String, Machine, Option<String>)> = None;
    // Los chips con `bios="…"`, hasta saber cuál es la predeterminada
    let mut chip_bios: Vec<Option<String>> = Vec::new();
    let mut in_header_version = false;
    let mut device_refs: HashMap<String, Vec<String>> = HashMap::new();

    loop {
        let event = reader.read_event_into(&mut buf).map_err(|e| format!("Not a valid DAT (XML): {}", e))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(event, Event::Empty(_));
                match e.name().as_ref() {
                    // MAME pone la versión en <mame build="0.289 (mame0289)">
                    b"mame" => {
                        if let Some(build) = attr(e, b"build") {
                            index.version = build.split_whitespace().next().unwrap_or("").to_string();
                        }
                    }
                    b"version" if !empty && index.version.is_empty() => in_header_version = true,
                    b"game" | b"machine" => {
                        let name = attr(e, b"name").unwrap_or_default();
                        let machine = Machine {
                            cloneof: attr(e, b"cloneof").filter(|s| !s.is_empty()),
                            romof: attr(e, b"romof").filter(|s| !s.is_empty()),
                            bios: attr(e, b"isbios").as_deref() == Some("yes"),
                            device: attr(e, b"isdevice").as_deref() == Some("yes"),
                            status: 3,
                            ..Default::default()
                        };
                        chip_bios.clear();
                        current = Some((name, machine, None));
                        if empty {
                            finish(&mut current, &mut chip_bios, &mut index);
                        }
                    }
                    b"rom" => {
                        if let Some((_, machine, _)) = current.as_mut() {
                            // Sin volcado: no se puede tener, no cuenta
                            let nodump = attr(e, b"status").as_deref() == Some("nodump");
                            let crc = attr(e, b"crc").and_then(|c| u32::from_str_radix(c.trim(), 16).ok());
                            if let (false, Some(crc)) = (nodump, crc) {
                                machine.chips.push(Chip {
                                    name: attr(e, b"name").unwrap_or_default(),
                                    crc,
                                    merged: attr(e, b"merge").is_some_and(|m| !m.is_empty()),
                                    optional: false,
                                });
                                chip_bios.push(attr(e, b"bios").filter(|b| !b.is_empty()));
                            }
                        }
                    }
                    b"disk" => {
                        if let Some((_, machine, _)) = current.as_mut() {
                            if attr(e, b"status").as_deref() != Some("nodump") {
                                if let Some(name) = attr(e, b"name").filter(|n| !n.is_empty()) {
                                    machine.disks.push(name);
                                }
                            }
                        }
                    }
                    b"biosset" => {
                        if let Some((_, _, default)) = current.as_mut() {
                            if attr(e, b"default").as_deref() == Some("yes") {
                                *default = attr(e, b"name");
                            }
                        }
                    }
                    b"driver" => {
                        if let Some((_, machine, _)) = current.as_mut() {
                            machine.status = match attr(e, b"status").as_deref() {
                                Some("good") => 0,
                                Some("imperfect") => 1,
                                Some("preliminary") => 2,
                                _ => 3,
                            };
                        }
                    }
                    b"device_ref" => {
                        if let (Some((name, _, _)), Some(device)) = (current.as_ref(), attr(e, b"name")) {
                            device_refs.entry(name.clone()).or_default().push(device);
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(t) if in_header_version => {
                index.version = t.unescape().map(|v| v.trim().to_string()).unwrap_or_default();
                in_header_version = false;
            }
            Event::End(ref e) => match e.name().as_ref() {
                b"game" | b"machine" => finish(&mut current, &mut chip_bios, &mut index),
                b"version" => in_header_version = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    // Solo los dispositivos que traen ROMs propias (la mayoría no tienen ninguna)
    for (name, refs) in device_refs {
        let with_roms: Vec<String> = refs
            .into_iter()
            .filter(|d| d != &name && index.machines.get(d).is_some_and(|m| m.device && !m.chips.is_empty()))
            .collect();
        if let Some(machine) = index.machines.get_mut(&name) {
            machine.devices = with_roms;
            machine.devices.dedup();
        }
    }
    if index.machines.is_empty() {
        return Err("The DAT has no games".into());
    }
    Ok(index)
}

/// Cierra la máquina que se estaba leyendo: los chips de otra BIOS que no sea la
/// predeterminada pasan a opcionales
fn finish(current: &mut Option<(String, Machine, Option<String>)>, chip_bios: &mut Vec<Option<String>>, index: &mut ArcadeIndex) {
    let Some((name, mut machine, default)) = current.take() else { return };
    for (chip, bios) in machine.chips.iter_mut().zip(chip_bios.iter()) {
        if let Some(bios) = bios {
            chip.optional = default.as_deref() != Some(bios.as_str());
        }
    }
    chip_bios.clear();
    if !name.is_empty() {
        index.machines.insert(name, machine);
    }
}

fn attr(e: &BytesStart, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find(|a| a.key.as_ref() == key).and_then(|a| a.unescape_value().ok().map(|v| v.to_string()))
}

// ---------- Diagnóstico ----------

/// Cómo está un set para un núcleo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetState {
    /// Todos los chips están (con el padre y la BIOS)
    Ok,
    /// Los chips del juego están, pero le falta algo de fuera: BIOS, padre, dispositivo o CHD
    Needs,
    /// Faltan chips del propio juego o no coinciden: es de otra versión de MAME
    Wrong,
    /// Ese núcleo no tiene el juego
    Absent,
    /// Es una BIOS o un dispositivo, no un juego
    NotAGame,
}

/// El diagnóstico de un set en un núcleo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetCheck {
    pub state: SetState,
    /// good | imperfect | preliminary | «» (el DAT no lo dice)
    pub driver: String,
    /// BIOS que falta o que no coincide (`neogeo`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bios: Option<String>,
    /// Padre que falta (`sf2`) en un set que lo necesita
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<String>,
    /// CHD que faltan (`<carpeta del juego>/<nombre>.chd`)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chd: Vec<String>,
    /// Chips del propio juego que faltan o no coinciden, de cuántos
    pub bad_chips: usize,
    pub chips: usize,
    /// Algunos chips que faltan (para enseñarlos)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_chips: Vec<String>,
}

impl SetCheck {
    fn new(state: SetState, driver: &str) -> Self {
        SetCheck {
            state,
            driver: driver.to_string(),
            bios: None,
            parent: None,
            devices: Vec::new(),
            chd: Vec::new(),
            bad_chips: 0,
            chips: 0,
            missing_chips: Vec::new(),
        }
    }
}

/// Lo que hay en disco alrededor del set: los CRC de un zip por su nombre (el del juego, el padre,
/// la BIOS…; `None` si no existe ni en la carpeta del sistema ni en la de BIOS) y si existe un CHD
pub trait SetFiles {
    fn crcs(&self, set_name: &str) -> Option<HashSet<u32>>;
    fn has_chd(&self, folder: &str, disk: &str) -> bool;
}

fn driver_name(status: u8) -> &'static str {
    match status {
        0 => "good",
        1 => "imperfect",
        2 => "preliminary",
        _ => "",
    }
}

/// Máximo de chips que faltan que se enseñan por nombre
const SHOWN_CHIPS: usize = 6;

/// Diagnostica el set `name` (el nombre del zip sin extensión) con el índice de un núcleo
pub fn diagnose(index: &ArcadeIndex, name: &str, files: &dyn SetFiles) -> SetCheck {
    let Some(machine) = index.machines.get(name) else { return SetCheck::new(SetState::Absent, "") };
    let driver = driver_name(machine.status);
    if machine.bios || machine.device {
        return SetCheck::new(SetState::NotAGame, driver);
    }
    let mut check = SetCheck::new(SetState::Ok, driver);
    let own = files.crcs(name).unwrap_or_default();

    // La BIOS: el final de la cadena `romof` que es una BIOS (neogeo, naomi, awbios…)
    let bios_name = bios_of(index, machine);
    let bios_crcs: HashSet<u32> = bios_name.and_then(|b| index.machines.get(b)).map(|b| b.chips.iter().map(|c| c.crc).collect()).unwrap_or_default();
    if let Some(bios) = bios_name {
        // Basta con que esté y sea de esa BIOS (alguna coincide): cada núcleo usa una variante.
        // Vale también dentro del zip del juego (sets «non-merged» con la BIOS dentro)
        let shares = |crcs: &HashSet<u32>| crcs.intersection(&bios_crcs).next().is_some();
        if !files.crcs(bios).is_some_and(|f| shares(&f)) && !shares(&own) {
            check.bios = Some(bios.to_string());
        }
    }

    // El padre (en un clon): su zip, si lo hay
    let parent_name = machine.cloneof.as_deref().filter(|p| Some(*p) != bios_name && index.machines.contains_key(*p));
    let parent = parent_name.and_then(|p| files.crcs(p));
    let devices: Vec<(&String, Option<HashSet<u32>>)> = machine.devices.iter().map(|d| (d, files.crcs(d))).collect();
    let in_pool = |crc: u32| {
        own.contains(&crc) || parent.as_ref().is_some_and(|p| p.contains(&crc)) || devices.iter().any(|(_, d)| d.as_ref().is_some_and(|d| d.contains(&crc)))
    };

    // Los chips del juego (los de la BIOS ya se han mirado aparte)
    let mut parent_needed = false;
    for chip in machine.chips.iter().filter(|c| !c.optional && !bios_crcs.contains(&c.crc)) {
        check.chips += 1;
        if in_pool(chip.crc) {
            continue;
        }
        if chip.merged && parent_name.is_some() && parent.is_none() {
            // Set «split»: está en el zip del padre, que no está
            parent_needed = true;
        } else {
            check.bad_chips += 1;
            if check.missing_chips.len() < SHOWN_CHIPS {
                check.missing_chips.push(chip.name.clone());
            }
        }
    }
    if parent_needed {
        check.parent = parent_name.map(String::from);
    }

    // Dispositivos con ROMs (MAME actual): en su zip, en el del juego o en el del padre
    for (device, crcs) in &devices {
        let chips = index.machines.get(*device).map(|d| &d.chips[..]).unwrap_or(&[]);
        let all = chips.iter().filter(|c| !c.optional).all(|c| crcs.as_ref().is_some_and(|d| d.contains(&c.crc)) || in_pool(c.crc));
        if !all {
            check.devices.push((*device).clone());
        }
    }

    // CHD: en la carpeta del juego o en la del padre
    for disk in &machine.disks {
        if !files.has_chd(name, disk) && !parent_name.is_some_and(|p| files.has_chd(p, disk)) {
            check.chd.push(disk.clone());
        }
    }

    check.state = if check.bad_chips > 0 {
        SetState::Wrong
    } else if check.bios.is_some() || check.parent.is_some() || !check.devices.is_empty() || !check.chd.is_empty() {
        SetState::Needs
    } else {
        SetState::Ok
    };
    check
}

/// La BIOS de una máquina: sigue `romof` hasta una que sea BIOS
fn bios_of<'a>(index: &'a ArcadeIndex, machine: &'a Machine) -> Option<&'a str> {
    let mut next = machine.romof.as_deref();
    for _ in 0..4 {
        let name = next?;
        let m = index.machines.get(name)?;
        if m.bios {
            return Some(name);
        }
        next = m.romof.as_deref();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un MAME pequeño inventado: BIOS con dos variantes, un original, un clon (split), un juego
    /// con CHD y un dispositivo con ROM
    const MAME: &str = r#"<?xml version="1.0"?>
<mame build="0.250 (mame0250)">
	<machine name="neogeo" isbios="yes">
		<biosset name="euro" description="Europa" default="yes"/>
		<biosset name="us" description="USA"/>
		<rom name="euro.sp1" bios="euro" size="10" crc="000000e1"/>
		<rom name="us.sp1" bios="us" size="10" crc="000000e2"/>
		<rom name="sfix.sfx" size="10" crc="000000e3"/>
	</machine>
	<machine name="mslug" romof="neogeo">
		<biosset name="euro" description="Europa" default="yes"/>
		<rom name="euro.sp1" merge="euro.sp1" bios="euro" size="10" crc="000000e1"/>
		<rom name="us.sp1" merge="us.sp1" bios="us" size="10" crc="000000e2"/>
		<rom name="sfix.sfx" merge="sfix.sfx" size="10" crc="000000e3"/>
		<rom name="201-p1.bin" size="10" crc="00000201"/>
		<rom name="201-c1.bin" size="10" crc="00000202"/>
		<rom name="nada.bin" size="10" status="nodump"/>
		<driver status="good"/>
	</machine>
	<machine name="sf2">
		<rom name="sf2.30" size="10" crc="00000a01"/>
		<rom name="sf2.gfx" size="10" crc="00000a02"/>
		<device_ref name="qsound"/>
		<driver status="imperfect"/>
	</machine>
	<machine name="sf2ua" cloneof="sf2" romof="sf2">
		<rom name="sf2u.30" size="10" crc="00000b01"/>
		<rom name="sf2.gfx" merge="sf2.gfx" size="10" crc="00000a02"/>
		<device_ref name="qsound"/>
		<driver status="imperfect"/>
	</machine>
	<machine name="kinst">
		<rom name="kinst.u98" size="10" crc="00000c01"/>
		<disk name="kinst" sha1="aa"/>
		<driver status="preliminary"/>
	</machine>
	<machine name="qsound" isdevice="yes">
		<rom name="dl-1425.bin" size="10" crc="00000d01"/>
	</machine>
	<machine name="speaker" isdevice="yes"/>
</mame>"#;

    fn index() -> ArcadeIndex {
        parse_index(MAME.as_bytes()).unwrap()
    }

    /// Ficheros inventados: zips por nombre con sus CRC y CHD por carpeta/nombre
    #[derive(Default)]
    struct Disk {
        zips: HashMap<String, Vec<u32>>,
        chds: HashSet<(String, String)>,
    }

    impl Disk {
        fn zip(mut self, name: &str, crcs: &[u32]) -> Self {
            self.zips.insert(name.into(), crcs.to_vec());
            self
        }
    }

    impl SetFiles for Disk {
        fn crcs(&self, set_name: &str) -> Option<HashSet<u32>> {
            self.zips.get(set_name).map(|c| c.iter().copied().collect())
        }
        fn has_chd(&self, folder: &str, disk: &str) -> bool {
            self.chds.contains(&(folder.to_string(), disk.to_string()))
        }
    }

    #[test]
    fn reads_versions_bios_variants_devices_and_status() {
        let ix = index();
        assert_eq!(ix.version, "0.250");
        let mslug = &ix.machines["mslug"];
        assert_eq!(mslug.status, 0);
        // La BIOS de EE. UU. no es la predeterminada: opcional; el chip sin volcado no está
        assert!(mslug.chips.iter().find(|c| c.name == "us.sp1").unwrap().optional);
        assert!(!mslug.chips.iter().find(|c| c.name == "euro.sp1").unwrap().optional);
        assert_eq!(mslug.chips.len(), 5);
        // Solo los dispositivos con ROMs
        assert_eq!(ix.machines["sf2"].devices, vec!["qsound".to_string()]);
        assert_eq!(ix.machines["kinst"].disks, vec!["kinst".to_string()]);

        let fbneo = parse_index(
            "<datafile><header><name>FBNeo</name><version>1.0.0.03</version></header><game name=\"a\"><rom name=\"x\" size=\"1\" crc=\"0000000f\"/><driver status=\"good\"/></game></datafile>".as_bytes(),
        )
        .unwrap();
        assert_eq!(fbneo.version, "1.0.0.03");
        assert_eq!(fbneo.machines["a"].chips[0].crc, 0xf);
    }

    #[test]
    fn a_complete_set_with_its_bios_is_ok() {
        let disk = Disk::default().zip("mslug", &[0x201, 0x202]).zip("neogeo", &[0xe1, 0xe3]);
        let c = diagnose(&index(), "mslug", &disk);
        assert_eq!(c.state, SetState::Ok, "{c:?}");
        assert_eq!(c.driver, "good");
        assert_eq!(c.chips, 2);
    }

    #[test]
    fn without_the_bios_it_needs_it() {
        let disk = Disk::default().zip("mslug", &[0x201, 0x202]);
        let c = diagnose(&index(), "mslug", &disk);
        assert_eq!(c.state, SetState::Needs);
        assert_eq!(c.bios.as_deref(), Some("neogeo"));
        // Una BIOS que no es esa (ningún chip coincide) tampoco vale
        let disk = Disk::default().zip("mslug", &[0x201, 0x202]).zip("neogeo", &[0x999]);
        assert_eq!(diagnose(&index(), "mslug", &disk).bios.as_deref(), Some("neogeo"));
    }

    #[test]
    fn other_chips_mean_another_version() {
        let disk = Disk::default().zip("mslug", &[0x201, 0x777]).zip("neogeo", &[0xe1]);
        let c = diagnose(&index(), "mslug", &disk);
        assert_eq!(c.state, SetState::Wrong);
        assert_eq!((c.bad_chips, c.chips), (1, 2));
        assert_eq!(c.missing_chips, vec!["201-c1.bin".to_string()]);
    }

    #[test]
    fn a_split_clone_needs_its_parent_and_devices() {
        let ix = index();
        // Split: el clon solo trae su chip; el resto está en sf2.zip
        let split = Disk::default().zip("sf2ua", &[0xb01]).zip("qsound", &[0xd01]);
        let c = diagnose(&ix, "sf2ua", &split);
        assert_eq!(c.state, SetState::Needs);
        assert_eq!(c.parent.as_deref(), Some("sf2"));
        let with_parent = split.zip("sf2", &[0xa01, 0xa02]);
        assert_eq!(diagnose(&ix, "sf2ua", &with_parent).state, SetState::Ok);
        // Non-merged: todo dentro del zip del clon; el dispositivo, aparte
        let c = diagnose(&ix, "sf2ua", &Disk::default().zip("sf2ua", &[0xb01, 0xa02]));
        assert_eq!(c.state, SetState::Needs);
        assert_eq!(c.devices, vec!["qsound".to_string()]);
        // El dispositivo también vale dentro del propio zip
        assert_eq!(diagnose(&ix, "sf2ua", &Disk::default().zip("sf2ua", &[0xb01, 0xa02, 0xd01])).state, SetState::Ok);
    }

    #[test]
    fn chds_absent_games_and_bios_sets() {
        let ix = index();
        let mut disk = Disk::default().zip("kinst", &[0xc01]);
        let c = diagnose(&ix, "kinst", &disk);
        assert_eq!((c.state.clone(), c.chd.clone(), c.driver.as_str()), (SetState::Needs, vec!["kinst".to_string()], "preliminary"));
        disk.chds.insert(("kinst".into(), "kinst".into()));
        assert_eq!(diagnose(&ix, "kinst", &disk).state, SetState::Ok);
        assert_eq!(diagnose(&ix, "pacman", &disk).state, SetState::Absent);
        assert_eq!(diagnose(&ix, "neogeo", &disk).state, SetState::NotAGame);
    }
}

