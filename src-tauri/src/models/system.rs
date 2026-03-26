use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub name: String,
    pub full_name: String,
    pub path: String,
    pub has_gamelist: bool,
    pub game_count: usize,
    pub rom_count: usize,
}

/// Auto-generated from ES-DE-Directories repository
/// https://github.com/retrogamecorps/ES-DE-Directories
pub fn rom_extensions_map() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    m.insert("3do", vec![".7z", ".bin", ".chd", ".cue", ".iso", ".zip"]);
    m.insert("adam", vec![".1dd", ".7z", ".bin", ".col", ".cqi", ".cqm", ".d77", ".d88", ".ddp", ".dfi", ".dsk", ".hfe", ".imd", ".mfi", ".mfm", ".rom", ".td0", ".wav", ".zip"]);
    m.insert("amiga", vec![".7z", ".adf", ".adz", ".ccd", ".chd", ".cue", ".dms", ".fdi", ".hdf", ".hdz", ".ipf", ".iso", ".lha", ".m3u", ".mds", ".nrg", ".rp9", ".uae", ".zip"]);
    m.insert("amiga1200", vec![".7z", ".adf", ".adz", ".ccd", ".chd", ".cue", ".dms", ".fdi", ".hdf", ".hdz", ".ipf", ".iso", ".lha", ".m3u", ".mds", ".nrg", ".rp9", ".uae", ".zip"]);
    m.insert("amiga600", vec![".7z", ".adf", ".adz", ".ccd", ".chd", ".cue", ".dms", ".fdi", ".hdf", ".hdz", ".ipf", ".iso", ".lha", ".m3u", ".mds", ".nrg", ".rp9", ".uae", ".zip"]);
    m.insert("amigacd32", vec![".7z", ".adf", ".adz", ".ccd", ".chd", ".cue", ".dms", ".fdi", ".hdf", ".hdz", ".ipf", ".iso", ".lha", ".m3u", ".mds", ".nrg", ".rp9", ".uae", ".zip"]);
    m.insert("amstradcpc", vec![".7z", ".cdt", ".cpr", ".dsk", ".kcr", ".m3u", ".sna", ".tap", ".tar", ".voc", ".zip"]);
    m.insert("androidapps", vec![".app"]);
    m.insert("androidgames", vec![".app"]);
    m.insert("apple2", vec![".do", ".dsk", ".nib", ".po"]);
    m.insert("apple2gs", vec![".2mg", ".7z", ".zip"]);
    m.insert("arcade", vec![".7z", ".cmd", ".neo", ".zip"]);
    m.insert("arcadia", vec![".7z", ".bin", ".zip"]);
    m.insert("archimedes", vec![".1dd", ".360", ".7z", ".adf", ".adl", ".adm", ".ads", ".apd", ".bbc", ".chd", ".cqi", ".cqm", ".d77", ".d88", ".dfi", ".dsd", ".dsk", ".hfe", ".ima", ".imd", ".img", ".ipf", ".jfd", ".mfi", ".mfm", ".msa", ".ssd", ".st", ".td0", ".ufi", ".zip"]);
    m.insert("arduboy", vec![".7z", ".arduboy", ".hex", ".zip"]);
    m.insert("astrocde", vec![".7z", ".zip"]);
    m.insert("atari2600", vec![".7z", ".a26", ".bin", ".zip"]);
    m.insert("atari5200", vec![".7z", ".a52", ".atr", ".atx", ".bin", ".car", ".cas", ".cdm", ".com", ".rom", ".xex", ".xfd", ".zip"]);
    m.insert("atari7800", vec![".7z", ".a78", ".bin", ".zip"]);
    m.insert("atari800", vec![".7z", ".a52", ".atr", ".atx", ".bin", ".car", ".cas", ".cdm", ".com", ".rom", ".xex", ".xfd", ".zip"]);
    m.insert("atarijaguar", vec![".7z", ".abs", ".bin", ".cdi", ".cof", ".cue", ".j64", ".jag", ".prg", ".rom", ".zip"]);
    m.insert("atarilynx", vec![".7z", ".lnx", ".lyx", ".o", ".zip"]);
    m.insert("atarist", vec![".7z", ".dim", ".ipf", ".m3u", ".msa", ".st", ".stx", ".zip"]);
    m.insert("atarixe", vec![".7z", ".a52", ".atr", ".atx", ".bin", ".car", ".cas", ".cdm", ".com", ".rom", ".xex", ".xfd", ".zip"]);
    m.insert("atomiswave", vec![".7z", ".bin", ".dat", ".elf", ".lst", ".zip"]);
    m.insert("bbcmicro", vec![".7z", ".dsd", ".img", ".ssd", ".zip"]);
    m.insert("c64", vec![".7z", ".bin", ".cmd", ".crt", ".d2m", ".d4m", ".d64", ".d6z", ".d71", ".d7z", ".d80", ".d81", ".d82", ".d8z", ".g41", ".g4z", ".g64", ".g6z", ".gz", ".lnx", ".m3u", ".nbz", ".nib", ".p00", ".prg", ".t64", ".tap", ".vfl", ".vsf", ".x64", ".x6z", ".zip"]);
    m.insert("cdimono1", vec![".chd", ".cue", ".iso"]);
    m.insert("cdtv", vec![".7z", ".adf", ".adz", ".ccd", ".chd", ".cue", ".dms", ".fdi", ".hdf", ".hdz", ".ipf", ".iso", ".lha", ".m3u", ".mds", ".nrg", ".rp9", ".uae", ".zip"]);
    m.insert("chailove", vec![".7z", ".chai", ".chailove", ".zip"]);
    m.insert("channelf", vec![".7z", ".bin", ".chf", ".zip"]);
    m.insert("coco", vec![".cas", ".ccc", ".dsk", ".rom"]);
    m.insert("colecovision", vec![".7z", ".bin", ".cas", ".col", ".cv", ".dsk", ".m3u", ".mx1", ".mx2", ".ri", ".rom", ".sc", ".sg", ".zip"]);
    m.insert("consolearcade", vec![".7z", ".arcadedef", ".iso", ".zip"]);
    m.insert("cps", vec![".7z", ".zip"]);
    m.insert("cps1", vec![".7z", ".zip"]);
    m.insert("cps2", vec![".7z", ".zip"]);
    m.insert("cps3", vec![".7z", ".zip"]);
    m.insert("crvision", vec![".7z", ".bin", ".rom", ".zip"]);
    m.insert("daphne", vec![".7z", ".dirksimple", ".ogv", ".zip"]);
    m.insert("doom", vec![".iwad", ".pwad", ".wad"]);
    m.insert("dos", vec![".7z", ".bat", ".com", ".conf", ".cue", ".dosz", ".exe", ".img", ".iso", ".zip"]);
    m.insert("dragon32", vec![".7z", ".cas", ".ccc", ".dsk", ".rom", ".zip"]);
    m.insert("dreamcast", vec![".7z", ".cdi", ".chd", ".cue", ".dat", ".elf", ".gdi", ".iso", ".lst", ".m3u", ".zip"]);
    m.insert("easyrpg", vec![".easyrpg", ".zip"]);
    m.insert("electron", vec![".1dd", ".7z", ".adf", ".adl", ".adm", ".ads", ".bbc", ".bin", ".cqi", ".cqm", ".csw", ".d77", ".d88", ".dfi", ".dsd", ".dsk", ".hfe", ".imd", ".img", ".mfi", ".mfm", ".rom", ".ssd", ".td0", ".uef", ".wav", ".zip"]);
    m.insert("emulators", vec![".app"]);
    m.insert("epic", vec![".app"]);
    m.insert("famicom", vec![".3dsen", ".7z", ".fds", ".nes", ".unf", ".unif", ".zip"]);
    m.insert("fba", vec![".7z", ".iso", ".zip"]);
    m.insert("fbneo", vec![".7z", ".zip"]);
    m.insert("fds", vec![".7z", ".fds", ".nes", ".unf", ".unif", ".zip"]);
    m.insert("flash", vec![".ruf", ".swf"]);
    m.insert("fm7", vec![".1dd", ".7z", ".cqi", ".cqm", ".d77", ".d88", ".dfi", ".dsk", ".hfe", ".imd", ".mfi", ".mfm", ".t77", ".td0", ".wav", ".zip"]);
    m.insert("fmtowns", vec![".cdr", ".chd", ".cue", ".gdi", ".iso"]);
    m.insert("gamate", vec![".7z", ".bin", ".zip"]);
    m.insert("gameandwatch", vec![".7z", ".mgw", ".zip"]);
    m.insert("gamecom", vec![".7z", ".tgc", ".zip"]);
    m.insert("gamegear", vec![".68k", ".7z", ".bin", ".bms", ".chd", ".col", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".rom", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("gb", vec![".7z", ".bs", ".cgb", ".dmg", ".gb", ".gbc", ".sfc", ".sgb", ".smc", ".zip"]);
    m.insert("gba", vec![".7z", ".agb", ".bin", ".cgb", ".dmg", ".gb", ".gba", ".gbc", ".sgb", ".zip"]);
    m.insert("gbc", vec![".7z", ".bs", ".cgb", ".dmg", ".gb", ".gbc", ".sfc", ".sgb", ".smc", ".zip"]);
    m.insert("gc", vec![".7z", ".ciso", ".dff", ".dol", ".elf", ".gcm", ".gcz", ".iso", ".json", ".m3u", ".rvz", ".tgc", ".wad", ".wbfs", ".wia", ".zip"]);
    m.insert("genesis", vec![".32x", ".68k", ".7z", ".bin", ".bms", ".chd", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("gmaster", vec![".7z", ".bin", ".zip"]);
    m.insert("gx4000", vec![".7z", ".bin", ".cdt", ".cpr", ".dsk", ".kcr", ".m3u", ".sna", ".tap", ".tar", ".voc", ".zip"]);
    m.insert("intellivision", vec![".7z", ".bin", ".int", ".rom", ".zip"]);
    m.insert("j2me", vec![".7z", ".jar", ".zip"]);
    m.insert("laserdisc", vec![".7z", ".dirksimple", ".ogv", ".zip"]);
    m.insert("lcdgames", vec![".7z", ".mgw", ".zip"]);
    m.insert("lowresnx", vec![".nx"]);
    m.insert("lutro", vec![".7z", ".lua", ".lutro", ".zip"]);
    m.insert("macintosh", vec![".dsk"]);
    m.insert("mame", vec![".7z", ".cmd", ".neo", ".zip"]);
    m.insert("mark3", vec![".68k", ".7z", ".bin", ".bms", ".chd", ".col", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".rom", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("mastersystem", vec![".68k", ".7z", ".bin", ".bms", ".chd", ".col", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".rom", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("megacd", vec![".68k", ".7z", ".bin", ".bms", ".chd", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("megacdjp", vec![".68k", ".7z", ".bin", ".bms", ".chd", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("megadrive", vec![".32x", ".68k", ".7z", ".bin", ".bms", ".chd", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("megadrivejp", vec![".32x", ".68k", ".7z", ".bin", ".bms", ".chd", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("megaduck", vec![".7z", ".bin", ".zip"]);
    m.insert("mess", vec![".7z", ".chd", ".zip"]);
    m.insert("model2", vec![".7z", ".zip"]);
    m.insert("model3", vec![".7z", ".zip"]);
    m.insert("moto", vec![".7z", ".fd", ".k7", ".m5", ".m7", ".rom", ".sap", ".zip"]);
    m.insert("msx", vec![".7z", ".cas", ".col", ".di1", ".di2", ".dmk", ".dsk", ".fd1", ".fd2", ".m3u", ".mx1", ".mx2", ".ogv", ".ri", ".rom", ".sc", ".sg", ".wav", ".xsa", ".zip"]);
    m.insert("msx1", vec![".7z", ".cas", ".col", ".di1", ".di2", ".dmk", ".dsk", ".fd1", ".fd2", ".m3u", ".mx1", ".mx2", ".ogv", ".ri", ".rom", ".sc", ".sg", ".wav", ".xsa", ".zip"]);
    m.insert("msx2", vec![".7z", ".cas", ".col", ".di1", ".di2", ".dmk", ".dsk", ".fd1", ".fd2", ".m3u", ".mx1", ".mx2", ".ogv", ".ri", ".rom", ".sc", ".sg", ".wav", ".xsa", ".zip"]);
    m.insert("msxturbor", vec![".7z", ".cas", ".col", ".di1", ".di2", ".dmk", ".dsk", ".fd1", ".fd2", ".m3u", ".mx1", ".mx2", ".ogv", ".ri", ".rom", ".sc", ".sg", ".wav", ".xsa", ".zip"]);
    m.insert("multivision", vec![".7z", ".bin", ".gg", ".rom", ".sg", ".sms", ".zip"]);
    m.insert("n3ds", vec![".3ds", ".3dsx", ".7z", ".app", ".axf", ".cci", ".cxi", ".elf", ".z3dsx", ".zcci", ".zcxi", ".zip"]);
    m.insert("n64", vec![".7z", ".app", ".bin", ".d64", ".n64", ".ndd", ".u1", ".v64", ".z64", ".zip"]);
    m.insert("n64dd", vec![".7z", ".bin", ".d64", ".n64", ".ndd", ".u1", ".v64", ".z64", ".zip"]);
    m.insert("naomi", vec![".7z", ".bin", ".dat", ".elf", ".lst", ".zip"]);
    m.insert("naomi2", vec![".7z", ".bin", ".dat", ".elf", ".lst", ".zip"]);
    m.insert("naomigd", vec![".7z", ".bin", ".dat", ".elf", ".lst", ".zip"]);
    m.insert("nds", vec![".7z", ".app", ".bin", ".nds", ".zip"]);
    m.insert("neogeo", vec![".7z", ".neo", ".zip"]);
    m.insert("neogeocd", vec![".chd", ".cue"]);
    m.insert("neogeocdjp", vec![".chd", ".cue"]);
    m.insert("nes", vec![".3dsen", ".7z", ".fds", ".nes", ".unf", ".unif", ".zip"]);
    m.insert("ngage", vec![".json"]);
    m.insert("ngp", vec![".7z", ".ngc", ".ngp", ".ngpc", ".npc", ".zip"]);
    m.insert("ngpc", vec![".7z", ".ngc", ".ngp", ".ngpc", ".npc", ".zip"]);
    m.insert("odyssey2", vec![".7z", ".bin", ".zip"]);
    m.insert("openbor", vec![".openbor"]);
    m.insert("oric", vec![".dsk", ".ort", ".tap", ".wav"]);
    m.insert("palm", vec![".7z", ".img", ".pqa", ".prc", ".zip"]);
    m.insert("pc", vec![".7z", ".bat", ".com", ".conf", ".cue", ".dosz", ".exe", ".img", ".iso", ".zip"]);
    m.insert("pc88", vec![".88d", ".cmt", ".d88", ".m3u", ".t88", ".u88"]);
    m.insert("pc98", vec![".2hd", ".7z", ".88d", ".98d", ".cmd", ".d88", ".d98", ".dup", ".fdd", ".fdi", ".hdd", ".hdi", ".hdm", ".hdn", ".m3u", ".nhd", ".tfd", ".thd", ".xdf", ".zip"]);
    m.insert("pcarcade", vec![".desktop"]);
    m.insert("pcengine", vec![".7z", ".ccd", ".chd", ".cue", ".img", ".iso", ".m3u", ".pce", ".rom", ".sgx", ".toc", ".zip"]);
    m.insert("pcenginecd", vec![".7z", ".ccd", ".chd", ".cue", ".img", ".iso", ".m3u", ".pce", ".sgx", ".toc", ".zip"]);
    m.insert("pcfx", vec![".7z", ".ccd", ".chd", ".cue", ".m3u", ".toc", ".zip"]);
    m.insert("pico8", vec![".p8", ".png"]);
    m.insert("plus4", vec![".7z", ".bin", ".cmd", ".crt", ".d2m", ".d4m", ".d64", ".d6z", ".d71", ".d7z", ".d80", ".d81", ".d82", ".d8z", ".g41", ".g4z", ".g64", ".g6z", ".gz", ".lnx", ".m3u", ".nbz", ".nib", ".p00", ".prg", ".t64", ".tap", ".vfl", ".vsf", ".x64", ".x6z", ".zip"]);
    m.insert("pokemini", vec![".7z", ".min", ".zip"]);
    m.insert("ports", vec![".exe", ".game", ".phd", ".psx"]);
    m.insert("ps2", vec![".bin", ".chd", ".ciso", ".cso", ".dump", ".elf", ".gz", ".img", ".iso", ".isz", ".m3u", ".mdf", ".ngr", ".nrg"]);
    m.insert("ps3", vec![".iso", ".ps3"]);
    m.insert("psp", vec![".7z", ".chd", ".cso", ".elf", ".iso", ".pbp", ".prx", ".zip"]);
    m.insert("psvita", vec![".psvita"]);
    m.insert("psx", vec![".7z", ".bin", ".cbn", ".ccd", ".chd", ".cue", ".ecm", ".exe", ".img", ".iso", ".m3u", ".mdf", ".mds", ".minipsf", ".pbp", ".psexe", ".psf", ".toc", ".z", ".zip", ".znx"]);
    m.insert("pv1000", vec![".7z", ".bin", ".zip"]);
    m.insert("quake", vec![".pak", ".pk3"]);
    m.insert("samcoupe", vec![".7z", ".dsk", ".mgt", ".sad", ".sbt", ".zip"]);
    m.insert("satellaview", vec![".7z", ".bml", ".bs", ".fig", ".sfc", ".smc", ".st", ".swc", ".zip"]);
    m.insert("saturn", vec![".7z", ".bin", ".ccd", ".chd", ".cue", ".iso", ".m3u", ".mds", ".toc", ".zip"]);
    m.insert("saturnjp", vec![".7z", ".bin", ".ccd", ".chd", ".cue", ".iso", ".m3u", ".mds", ".toc", ".zip"]);
    m.insert("scummvm", vec![".scummvm", ".svm"]);
    m.insert("scv", vec![".0", ".7z", ".bin", ".zip"]);
    m.insert("sega32x", vec![".32x", ".68k", ".7z", ".bin", ".chd", ".cue", ".gen", ".iso", ".m3u", ".md", ".smd", ".sms", ".zip"]);
    m.insert("sega32xjp", vec![".32x", ".68k", ".7z", ".bin", ".chd", ".cue", ".gen", ".iso", ".m3u", ".md", ".smd", ".sms", ".zip"]);
    m.insert("sega32xna", vec![".32x", ".68k", ".7z", ".bin", ".chd", ".cue", ".gen", ".iso", ".m3u", ".md", ".smd", ".sms", ".zip"]);
    m.insert("segacd", vec![".68k", ".7z", ".bin", ".bms", ".chd", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("sfc", vec![".7z", ".bin", ".bml", ".bs", ".bsx", ".dx2", ".fig", ".gd3", ".gd7", ".mgd", ".sfc", ".smc", ".st", ".swc", ".zip"]);
    m.insert("sg-1000", vec![".68k", ".7z", ".bin", ".bms", ".chd", ".cue", ".gen", ".gg", ".iso", ".m3u", ".md", ".mdx", ".ri", ".rom", ".sg", ".sgd", ".smd", ".sms", ".zip"]);
    m.insert("sgb", vec![".7z", ".gb", ".gbc", ".sgb", ".zip"]);
    m.insert("snes", vec![".7z", ".bin", ".bml", ".bs", ".bsx", ".dx2", ".fig", ".gd3", ".gd7", ".mgd", ".sfc", ".smc", ".st", ".swc", ".zip"]);
    m.insert("snesna", vec![".7z", ".bin", ".bml", ".bs", ".bsx", ".dx2", ".fig", ".gd3", ".gd7", ".mgd", ".sfc", ".smc", ".st", ".swc", ".zip"]);
    m.insert("spectravideo", vec![".7z", ".cas", ".col", ".dsk", ".m3u", ".mx1", ".mx2", ".ri", ".rom", ".sc", ".sg", ".zip"]);
    m.insert("steam", vec![".steam"]);
    m.insert("stv", vec![".7z", ".zip"]);
    m.insert("sufami", vec![".7z", ".bml", ".bs", ".fig", ".sfc", ".smc", ".st", ".zip"]);
    m.insert("supergrafx", vec![".7z", ".ccd", ".chd", ".cue", ".pce", ".rom", ".sgx", ".zip"]);
    m.insert("supervision", vec![".7z", ".bin", ".sv", ".zip"]);
    m.insert("supracan", vec![".7z", ".bin", ".zip"]);
    m.insert("switch", vec![".nca", ".nro", ".nso", ".nsp", ".xci"]);
    m.insert("symbian", vec![".json"]);
    m.insert("tanodragon", vec![".7z", ".cas", ".ccc", ".dsk", ".rom", ".zip"]);
    m.insert("tg-cd", vec![".7z", ".ccd", ".chd", ".cue", ".img", ".iso", ".m3u", ".pce", ".sgx", ".toc", ".zip"]);
    m.insert("tg16", vec![".7z", ".ccd", ".chd", ".cue", ".img", ".iso", ".m3u", ".pce", ".rom", ".sgx", ".toc", ".zip"]);
    m.insert("ti99", vec![".7z", ".rpk", ".zip"]);
    m.insert("tic80", vec![".tic"]);
    m.insert("to8", vec![".7z", ".fd", ".k7", ".m5", ".m7", ".rom", ".sap", ".zip"]);
    m.insert("type-x", vec![".desktop"]);
    m.insert("uzebox", vec![".7z", ".uze", ".zip"]);
    m.insert("vectrex", vec![".7z", ".bin", ".gam", ".vc", ".vec", ".zip"]);
    m.insert("vic20", vec![".7z", ".bin", ".cmd", ".crt", ".d2m", ".d4m", ".d64", ".d6z", ".d71", ".d7z", ".d80", ".d81", ".d82", ".d8z", ".g41", ".g4z", ".g64", ".g6z", ".gz", ".lnx", ".m3u", ".nbz", ".nib", ".p00", ".prg", ".t64", ".tap", ".vfl", ".vsf", ".x64", ".x6z", ".zip"]);
    m.insert("videopac", vec![".7z", ".bin", ".zip"]);
    m.insert("vircon32", vec![".7z", ".v32", ".zip"]);
    m.insert("virtualboy", vec![".7z", ".bin", ".vb", ".vboy", ".zip"]);
    m.insert("vpinball", vec![".vpinball", ".vpt", ".vpx"]);
    m.insert("vsmile", vec![".7z", ".bin", ".zip"]);
    m.insert("wasm4", vec![".wasm"]);
    m.insert("wii", vec![".7z", ".ciso", ".dff", ".dol", ".elf", ".gcm", ".gcz", ".iso", ".json", ".m3u", ".rvz", ".tgc", ".wad", ".wbfs", ".wia", ".zip"]);
    m.insert("wiiu", vec![".elf", ".rpx", ".tmd", ".wua", ".wud", ".wuhb", ".wux"]);
    m.insert("windows", vec![".desktop", ".steam"]);
    m.insert("windows3x", vec![".7z", ".bat", ".dosz", ".zip"]);
    m.insert("windows9x", vec![".7z", ".bat", ".dosz", ".zip"]);
    m.insert("wonderswan", vec![".7z", ".pc2", ".ws", ".zip"]);
    m.insert("wonderswancolor", vec![".7z", ".pc2", ".ws", ".wsc", ".zip"]);
    m.insert("x1", vec![".2d", ".2hd", ".7z", ".88d", ".cmd", ".d88", ".dup", ".dx1", ".hdm", ".tap", ".tfd", ".xdf", ".zip"]);
    m.insert("x68000", vec![".2hd", ".7z", ".88d", ".cmd", ".d88", ".dim", ".dup", ".hdf", ".hdm", ".img", ".m3u", ".xdf", ".zip"]);
    m.insert("zmachine", vec![".dat", ".z1", ".z2", ".z3", ".z4", ".z5", ".z6", ".z7", ".z8", ".zblorb", ".zlb"]);
    m.insert("zx81", vec![".7z", ".p", ".tzx", ".zip"]);
    m.insert("zxspectrum", vec![".7z", ".dsk", ".gz", ".img", ".mgt", ".rzx", ".scl", ".sh", ".sna", ".szx", ".tap", ".trd", ".tzx", ".udi", ".z80", ".zip"]);
    m
}

pub fn system_full_name(folder_name: &str) -> Option<&'static str> {
    match folder_name.to_lowercase().as_str() {
        "3do" => Some("3DO Interactive Multiplayer"),
        "adam" => Some("Coleco Adam"),
        "amiga" => Some("Commodore Amiga"),
        "amiga1200" => Some("Commodore Amiga 1200"),
        "amiga600" => Some("Commodore Amiga 600"),
        "amigacd32" => Some("Commodore Amiga CD32"),
        "amstradcpc" => Some("Amstrad CPC"),
        "androidapps" => Some("Android Apps"),
        "androidgames" => Some("Android Games"),
        "apple2" => Some("Apple II"),
        "apple2gs" => Some("Apple IIGS"),
        "arcade" => Some("Arcade"),
        "arcadia" => Some("Emerson Arcadia 2001"),
        "archimedes" => Some("Acorn Archimedes"),
        "arduboy" => Some("Arduboy Miniature Game System"),
        "astrocde" => Some("Bally Astrocade"),
        "atari2600" => Some("Atari 2600"),
        "atari5200" => Some("Atari 5200"),
        "atari7800" => Some("Atari 7800 ProSystem"),
        "atari800" => Some("Atari 800"),
        "atarijaguar" => Some("Atari Jaguar"),
        "atarilynx" => Some("Atari Lynx"),
        "atarist" => Some("Atari ST"),
        "atarixe" => Some("Atari XE"),
        "atomiswave" => Some("Sammy Corporation Atomiswave"),
        "bbcmicro" => Some("Acorn Computers BBC Micro"),
        "c64" => Some("Commodore 64"),
        "cdimono1" => Some("Philips CD-i"),
        "cdtv" => Some("Commodore CDTV"),
        "chailove" => Some("ChaiLove Game Engine"),
        "channelf" => Some("Fairchild Channel F"),
        "coco" => Some("Tandy Color Computer"),
        "colecovision" => Some("Coleco ColecoVision"),
        "consolearcade" => Some("Console Arcade Systems"),
        "cps" => Some("Capcom Play System"),
        "cps1" => Some("Capcom Play System I"),
        "cps2" => Some("Capcom Play System II"),
        "cps3" => Some("Capcom Play System III"),
        "crvision" => Some("VTech CreatiVision"),
        "daphne" => Some("Daphne Arcade LaserDisc Emulator"),
        "doom" => Some("Doom"),
        "dos" => Some("DOS (PC)"),
        "dragon32" => Some("Dragon Data Dragon 32"),
        "dreamcast" => Some("Sega Dreamcast"),
        "easyrpg" => Some("EasyRPG Game Engine"),
        "electron" => Some("Acorn Electron"),
        "emulators" => Some("Emulators"),
        "epic" => Some("Epic Games Store"),
        "famicom" => Some("Nintendo Family Computer"),
        "fba" => Some("FinalBurn Alpha"),
        "fbneo" => Some("FinalBurn Neo"),
        "fds" => Some("Nintendo Famicom Disk System"),
        "flash" => Some("Adobe Flash"),
        "fm7" => Some("Fujitsu FM-7"),
        "fmtowns" => Some("Fujitsu FM Towns"),
        "gamate" => Some("Bit Corporation Gamate"),
        "gameandwatch" => Some("Nintendo Game and Watch"),
        "gamecom" => Some("Tiger Electronics Game.com"),
        "gamegear" => Some("Sega Game Gear"),
        "gb" => Some("Nintendo Game Boy"),
        "gba" => Some("Nintendo Game Boy Advance"),
        "gbc" => Some("Nintendo Game Boy Color"),
        "gc" => Some("Nintendo GameCube"),
        "genesis" => Some("Sega Genesis"),
        "gmaster" => Some("Hartung Game Master"),
        "gx4000" => Some("Amstrad GX4000"),
        "intellivision" => Some("Mattel Electronics Intellivision"),
        "j2me" => Some("Java 2 Micro Edition (J2ME)"),
        "laserdisc" => Some("LaserDisc Games"),
        "lcdgames" => Some("LCD Handheld Games"),
        "lowresnx" => Some("LowRes NX Fantasy Console"),
        "lutro" => Some("Lutro Game Engine"),
        "macintosh" => Some("Apple Macintosh"),
        "mame" => Some("Multiple Arcade Machine Emulator"),
        "mark3" => Some("Sega Mark III"),
        "mastersystem" => Some("Sega Master System"),
        "megacd" => Some("Sega Mega-CD"),
        "megacdjp" => Some("Sega Mega-CD"),
        "megadrive" => Some("Sega Mega Drive"),
        "megadrivejp" => Some("Sega Mega Drive"),
        "megaduck" => Some("Creatronic Mega Duck"),
        "mess" => Some("Multi Emulator Super System"),
        "model2" => Some("Sega Model 2"),
        "model3" => Some("Sega Model 3"),
        "moto" => Some("Thomson MO/TO Series"),
        "msx" => Some("MSX"),
        "msx1" => Some("MSX1"),
        "msx2" => Some("MSX2"),
        "msxturbor" => Some("MSX Turbo R"),
        "multivision" => Some("Othello Multivision"),
        "n3ds" => Some("Nintendo 3DS"),
        "n64" => Some("Nintendo 64"),
        "n64dd" => Some("Nintendo 64DD"),
        "naomi" => Some("Sega NAOMI"),
        "naomi2" => Some("Sega NAOMI 2"),
        "naomigd" => Some("Sega NAOMI GD-ROM"),
        "nds" => Some("Nintendo DS"),
        "neogeo" => Some("SNK Neo Geo"),
        "neogeocd" => Some("SNK Neo Geo CD"),
        "neogeocdjp" => Some("SNK Neo Geo CD"),
        "nes" => Some("Nintendo Entertainment System"),
        "ngage" => Some("Nokia N-Gage"),
        "ngp" => Some("SNK Neo Geo Pocket"),
        "ngpc" => Some("SNK Neo Geo Pocket Color"),
        "odyssey2" => Some("Magnavox Odyssey 2"),
        "openbor" => Some("OpenBOR Game Engine"),
        "oric" => Some("Tangerine Computer Systems Oric"),
        "palm" => Some("Palm OS"),
        "pc" => Some("IBM PC"),
        "pc88" => Some("NEC PC-8800 Series"),
        "pc98" => Some("NEC PC-9800 Series"),
        "pcarcade" => Some("PC Arcade Systems"),
        "pcengine" => Some("NEC PC Engine"),
        "pcenginecd" => Some("NEC PC Engine CD"),
        "pcfx" => Some("NEC PC-FX"),
        "pico8" => Some("PICO-8 Fantasy Console"),
        "plus4" => Some("Commodore Plus/4"),
        "pokemini" => Some("Nintendo Pokémon Mini"),
        "ports" => Some("Ports"),
        "ps2" => Some("Sony PlayStation 2"),
        "ps3" => Some("Sony PlayStation 3"),
        "psp" => Some("Sony PlayStation Portable"),
        "psvita" => Some("Sony PlayStation Vita"),
        "psx" => Some("Sony PlayStation"),
        "pv1000" => Some("Casio PV-1000"),
        "quake" => Some("Quake"),
        "samcoupe" => Some("MGT SAM Coupé"),
        "satellaview" => Some("Nintendo Satellaview"),
        "saturn" => Some("Sega Saturn"),
        "saturnjp" => Some("Sega Saturn"),
        "scummvm" => Some("ScummVM Game Engine"),
        "scv" => Some("Epoch Super Cassette Vision"),
        "sega32x" => Some("Sega Mega Drive 32X"),
        "sega32xjp" => Some("Sega Super 32X"),
        "sega32xna" => Some("Sega Genesis 32X"),
        "segacd" => Some("Sega CD"),
        "sfc" => Some("Nintendo SFC (Super Famicom)"),
        "sg-1000" => Some("Sega SG-1000"),
        "sgb" => Some("Nintendo Super Game Boy"),
        "snes" => Some("Nintendo SNES (Super Nintendo)"),
        "snesna" => Some("Nintendo SNES (Super Nintendo)"),
        "spectravideo" => Some("Spectravideo"),
        "steam" => Some("Valve Steam"),
        "stv" => Some("Sega Titan Video Game System"),
        "sufami" => Some("Bandai SuFami Turbo"),
        "supergrafx" => Some("NEC SuperGrafx"),
        "supervision" => Some("Watara Supervision"),
        "supracan" => Some(""),
        "switch" => Some("Nintendo Switch"),
        "symbian" => Some("Symbian"),
        "tanodragon" => Some("Tano Dragon"),
        "tg-cd" => Some("NEC TurboGrafx-CD"),
        "tg16" => Some("NEC TurboGrafx-16"),
        "ti99" => Some("Texas Instruments TI-99"),
        "tic80" => Some("TIC-80 Fantasy Computer"),
        "to8" => Some("Thomson TO8"),
        "type-x" => Some("Taito Type X"),
        "uzebox" => Some("Uzebox Open Source Console"),
        "vectrex" => Some("GCE Vectrex"),
        "vic20" => Some("Commodore VIC-20"),
        "videopac" => Some("Philips Videopac G7000"),
        "vircon32" => Some("Vircon32 Virtual Console"),
        "virtualboy" => Some("Nintendo Virtual Boy"),
        "vpinball" => Some("Visual Pinball"),
        "vsmile" => Some("VTech V.Smile"),
        "wasm4" => Some("WASM-4 Fantasy Console"),
        "wii" => Some("Nintendo Wii"),
        "wiiu" => Some("Nintendo Wii U"),
        "windows" => Some("Microsoft Windows"),
        "windows3x" => Some("Microsoft Windows 3.x"),
        "windows9x" => Some("Microsoft Windows 9x"),
        "wonderswan" => Some("Bandai WonderSwan"),
        "wonderswancolor" => Some("Bandai WonderSwan Color"),
        "x1" => Some("Sharp X1"),
        "x68000" => Some("Sharp X68000"),
        "zmachine" => Some("Infocom Z-machine"),
        "zx81" => Some("Sinclair ZX81"),
        "zxspectrum" => Some("Sinclair ZX Spectrum"),
        _ => None,
    }
}

pub fn scraping_platform(folder_name: &str) -> Option<&'static str> {
    match folder_name.to_lowercase().as_str() {
        "3do" => Some("3do"),
        "adam" => Some("adam"),
        "amiga" => Some("amiga"),
        "amiga1200" => Some("amiga"),
        "amiga600" => Some("amiga"),
        "amigacd32" => Some("amigacd32"),
        "amstradcpc" => Some("amstradcpc"),
        "androidapps" => Some("android"),
        "androidgames" => Some("android"),
        "apple2" => Some("apple2"),
        "apple2gs" => Some("apple2gs"),
        "arcade" => Some("arcade"),
        "arcadia" => Some("arcadia"),
        "archimedes" => Some("archimedes"),
        "arduboy" => Some("arduboy"),
        "astrocde" => Some("astrocde"),
        "atari2600" => Some("atari2600"),
        "atari5200" => Some("atari5200"),
        "atari7800" => Some("atari7800"),
        "atari800" => Some("atari800"),
        "atarijaguar" => Some("atarijaguar"),
        "atarilynx" => Some("atarilynx"),
        "atarist" => Some("atarist"),
        "atarixe" => Some("atarixe"),
        "atomiswave" => Some("arcade"),
        "bbcmicro" => Some("bbcmicro"),
        "c64" => Some("c64"),
        "cdimono1" => Some("cdimono1"),
        "cdtv" => Some("cdtv"),
        "chailove" => Some("love"),
        "channelf" => Some("channelf"),
        "coco" => Some("coco"),
        "colecovision" => Some("colecovision"),
        "consolearcade" => Some("arcade"),
        "cps" => Some("arcade"),
        "cps1" => Some("arcade"),
        "cps2" => Some("arcade"),
        "cps3" => Some("arcade"),
        "crvision" => Some("crvision"),
        "dos" => Some("dos"),
        "dragon32" => Some("dragon32"),
        "dreamcast" => Some("dreamcast"),
        "easyrpg" => Some("easyrpg"),
        "electron" => Some("electron"),
        "emulators" => Some("android"),
        "epic" => Some("android"),
        "famicom" => Some("famicom"),
        "fba" => Some("arcade"),
        "fbneo" => Some("arcade"),
        "fds" => Some("fds"),
        "flash" => Some("flash"),
        "fm7" => Some("fm7"),
        "fmtowns" => Some("fmtowns"),
        "gamate" => Some("gamate"),
        "gameandwatch" => Some("gameandwatch"),
        "gamecom" => Some("gamecom"),
        "gamegear" => Some("gamegear"),
        "gb" => Some("gb"),
        "gba" => Some("gba"),
        "gbc" => Some("gbc"),
        "gc" => Some("gc"),
        "genesis" => Some("genesis"),
        "gmaster" => Some("gmaster"),
        "gx4000" => Some("gx4000"),
        "intellivision" => Some("intellivision"),
        "j2me" => Some("android"),
        "lowresnx" => Some("lowresnx"),
        "lutro" => Some("lutro"),
        "macintosh" => Some("macintosh"),
        "mame" => Some("arcade"),
        "mark3" => Some("mastersystem"),
        "mastersystem" => Some("mastersystem"),
        "megacd" => Some("segacd"),
        "megacdjp" => Some("segacd"),
        "megadrive" => Some("megadrive"),
        "megadrivejp" => Some("megadrive"),
        "megaduck" => Some("megaduck"),
        "mess" => Some("mess"),
        "model2" => Some("arcade"),
        "model3" => Some("arcade"),
        "moto" => Some("moto"),
        "msx" => Some("msx"),
        "msx1" => Some("msx"),
        "msx2" => Some("msx2"),
        "msxturbor" => Some("msxturbor"),
        "multivision" => Some("sg-1000"),
        "n3ds" => Some("n3ds"),
        "n64" => Some("n64"),
        "n64dd" => Some("n64"),
        "naomi" => Some("arcade"),
        "naomi2" => Some("arcade"),
        "naomigd" => Some("arcade"),
        "nds" => Some("nds"),
        "neogeo" => Some("neogeo"),
        "neogeocd" => Some("neogeocd"),
        "neogeocdjp" => Some("neogeocd"),
        "nes" => Some("nes"),
        "ngage" => Some("ngage"),
        "ngp" => Some("ngp"),
        "ngpc" => Some("ngpc"),
        "odyssey2" => Some("odyssey2"),
        "openbor" => Some("openbor"),
        "oric" => Some("oric"),
        "palm" => Some("palm"),
        "pc" => Some("pc"),
        "pc88" => Some("pc88"),
        "pc98" => Some("pc98"),
        "pcarcade" => Some("arcade"),
        "pcengine" => Some("pcengine"),
        "pcenginecd" => Some("pcenginecd"),
        "pcfx" => Some("pcfx"),
        "pico8" => Some("pico8"),
        "plus4" => Some("plus4"),
        "pokemini" => Some("pokemini"),
        "ps2" => Some("ps2"),
        "ps3" => Some("ps3"),
        "psp" => Some("psp"),
        "psvita" => Some("psvita"),
        "psx" => Some("psx"),
        "pv1000" => Some("pv1000"),
        "samcoupe" => Some("samcoupe"),
        "satellaview" => Some("satellaview"),
        "saturn" => Some("saturn"),
        "saturnjp" => Some("saturn"),
        "scummvm" => Some("scummvm"),
        "scv" => Some("scv"),
        "sega32x" => Some("sega32x"),
        "sega32xjp" => Some("sega32x"),
        "sega32xna" => Some("sega32x"),
        "segacd" => Some("segacd"),
        "sfc" => Some("snes"),
        "sg-1000" => Some("sg-1000"),
        "sgb" => Some("sgb"),
        "snes" => Some("snes"),
        "snesna" => Some("snes"),
        "spectravideo" => Some("spectravideo"),
        "steam" => Some("steam"),
        "stv" => Some("arcade"),
        "sufami" => Some("sufami"),
        "supergrafx" => Some("supergrafx"),
        "supervision" => Some("supervision"),
        "supracan" => Some("supracan"),
        "switch" => Some("switch"),
        "symbian" => Some("ngage"),
        "tanodragon" => Some("dragon32"),
        "tg-cd" => Some("pcenginecd"),
        "tg16" => Some("pcengine"),
        "ti99" => Some("ti99"),
        "tic80" => Some("tic80"),
        "to8" => Some("moto"),
        "type-x" => Some("arcade"),
        "uzebox" => Some("uzebox"),
        "vectrex" => Some("vectrex"),
        "vic20" => Some("vic20"),
        "videopac" => Some("odyssey2"),
        "vircon32" => Some("vircon32"),
        "virtualboy" => Some("virtualboy"),
        "vpinball" => Some("vpinball"),
        "vsmile" => Some("vsmile"),
        "wasm4" => Some("wasm4"),
        "wii" => Some("wii"),
        "wiiu" => Some("wiiu"),
        "windows" => Some("pcwindows"),
        "windows3x" => Some("windows3x"),
        "windows9x" => Some("pcwindows"),
        "wonderswan" => Some("wonderswan"),
        "wonderswancolor" => Some("wonderswancolor"),
        "x1" => Some("x1"),
        "x68000" => Some("x68000"),
        "zmachine" => Some("zmachine"),
        "zx81" => Some("zx81"),
        "zxspectrum" => Some("zxspectrum"),
        _ => None,
    }
}

/// Given a system folder name, returns its known ROM extensions.
/// Falls back to common archive extensions if the system is unknown.
pub fn extensions_for_system(folder_name: &str) -> Vec<&'static str> {
    let map = rom_extensions_map();
    let key = folder_name.to_lowercase();
    map.get(key.as_str()).cloned().unwrap_or_else(|| {
        vec![".zip", ".7z", ".bin", ".iso", ".rom"]
    })
}

/// Maps ScreenScraper platform names to their numeric system IDs.
fn platform_to_screenscraper_id(platform: &str) -> Option<u32> {
    match platform {
        "3do" => Some(29),
        "adam" => Some(89),
        "amiga" => Some(64),
        "amigacd32" => Some(130),
        "amstradcpc" => Some(65),
        "apple2" => Some(86),
        "apple2gs" => Some(217),
        "arcade" => Some(75),
        "arcadia" => Some(94),
        "astrocde" => Some(44),
        "atari2600" => Some(26),
        "atari5200" => Some(40),
        "atari7800" => Some(41),
        "atari800" => Some(43),
        "atarijaguar" => Some(27),
        "atarilynx" => Some(28),
        "atarist" => Some(42),
        "atarixe" => Some(43),
        "atomiswave" => Some(53),
        "bbcmicro" => Some(37),
        "c64" => Some(66),
        "cdimono1" | "cdi" => Some(133),
        "channelf" => Some(80),
        "colecovision" => Some(48),
        "cps" | "cps1" | "cps2" | "cps3" => Some(75),
        "daphne" => Some(49),
        "dos" | "pc" => Some(135),
        "dreamcast" => Some(23),
        "famicom" => Some(3),
        "fba" | "fbneo" => Some(75),
        "fds" => Some(106),
        "fm7" => Some(97),
        "fmtowns" => Some(253),
        "gamegear" => Some(21),
        "gameandwatch" => Some(52),
        "gb" => Some(9),
        "gba" => Some(12),
        "gbc" => Some(10),
        "gc" | "gamecube" => Some(13),
        "genesis" | "megadrive" => Some(1),
        "intellivision" => Some(115),
        "mame" => Some(75),
        "mastersystem" | "mark3" => Some(2),
        "megacd" | "segacd" => Some(20),
        "msx" | "msx1" => Some(113),
        "msx2" => Some(116),
        "msxturbor" => Some(118),
        "n3ds" | "3ds" => Some(17),
        "n64" => Some(14),
        "naomi" => Some(56),
        "naomi2" => Some(230),
        "naomigd" => Some(55),
        "nds" => Some(15),
        "neogeo" => Some(142),
        "neogeocd" => Some(70),
        "nes" => Some(3),
        "ngp" => Some(25),
        "ngpc" => Some(82),
        "odyssey2" | "videopac" => Some(104),
        "oric" => Some(131),
        "pc88" => Some(221),
        "pc98" => Some(208),
        "pcengine" | "tg16" | "supergrafx" => Some(31),
        "pcenginecd" | "tg-cd" => Some(114),
        "pcfx" => Some(72),
        "pico8" => Some(234),
        "pokemini" => Some(211),
        "ps2" => Some(58),
        "ps3" => Some(59),
        "psp" => Some(61),
        "psvita" => Some(62),
        "psx" => Some(57),
        "saturn" | "saturnjp" => Some(22),
        "scummvm" => Some(123),
        "sega32x" | "sega32xjp" | "sega32xna" => Some(19),
        "sg-1000" => Some(109),
        "snes" | "snesna" | "sfc" => Some(4),
        "stv" => Some(54),
        "switch" => Some(225),
        "ti99" => Some(205),
        "vectrex" => Some(102),
        "vic20" => Some(73),
        "virtualboy" => Some(11),
        "wii" => Some(16),
        "wiiu" => Some(18),
        "wonderswan" => Some(45),
        "wonderswancolor" => Some(46),
        "x1" => Some(220),
        "x68000" => Some(79),
        "zx81" => Some(77),
        "zxspectrum" => Some(76),
        _ => None,
    }
}

/// Get the ScreenScraper system ID for a folder name.
/// Uses the scraping platform from ES-DE data, then maps to numeric ID.
pub fn screenscraper_id_for_system(folder_name: &str) -> Option<u32> {
    // First try direct folder name
    if let Some(id) = platform_to_screenscraper_id(&folder_name.to_lowercase()) {
        return Some(id);
    }
    // Then try via the scraping platform
    if let Some(platform) = scraping_platform(folder_name) {
        return platform_to_screenscraper_id(platform);
    }
    None
}
