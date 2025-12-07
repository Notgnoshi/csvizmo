use pretty_assertions::assert_eq;

use crate::{CommandExt, tool};

#[test]
fn test_cli_candump_format() {
    let input = b"\
        (1740007472.187687)  can0  0D15F192   [8]  50 0B 37 66 CB 2D ED 7C\n\
        (1740007483.553746)  can0  0E9790B5   [8]  CA 3F 87 1A 5A 6E E7 5F\n\
    ";
    let expected = "\
        timestamp,interface,canid,dlc,priority,src,dst,pgn,data\n\
        1740007472.187687,can0,0xD15F192,8,3,0x92,0xF1,0x11500,500B3766CB2DED7C\n\
        1740007483.553746,can0,0xE9790B5,8,3,0xB5,0x90,0x29700,CA3F871A5A6EE75F\n\
    ";
    let output = tool("can2csv")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}

#[test]
fn test_file_candump_format() {
    let input = b"\
        (1739229594.465994) can0 0E9790B5#CA3F871A5A6EE75F\n\
        (1739229594.467052) can0 0D15F192#500B3766CB2DED7C\n\
    ";
    let expected = "\
        timestamp,interface,canid,dlc,priority,src,dst,pgn,data\n\
        1739229594.465994,can0,0xE9790B5,8,3,0xB5,0x90,0x29700,CA3F871A5A6EE75F\n\
        1739229594.467052,can0,0xD15F192,8,3,0x92,0xF1,0x11500,500B3766CB2DED7C\n\
    ";
    let output = tool("can2csv")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}

#[test]
fn test_fastpacket_reconstruction() {
    let input = b"\
        (1739920494.579828) can0 15F805FE#A012010203040506\n\
        (1739920494.580925) can0 15F805FE#A10708090A0B0C0D\n\
        (1739920494.582015) can0 15F805FE#A20E0F101112\n\
    ";
    let expected = "\
        timestamp,interface,canid,dlc,priority,src,dst,pgn,data\n\
        1739920494.579828,can0,0x15F805FE,18,5,0xFE,0xFF,0x1F805,0102030405060708090A0B0C0D0E0F101112\n\
    ";
    let mut cmd = tool("can2csv");
    cmd.arg("--reconstruct");
    let output = cmd.write_stdin(input).captured_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}
