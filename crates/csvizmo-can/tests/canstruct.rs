use csvizmo_test::{CommandExt, tool};
use pretty_assertions::assert_eq;

#[test]
fn test_single_fp_tp() {
    let input = b"\
        (1739920494.579828) can0 15F805FE#A012010203040506                          \n\
        (1739920494.580925) can0 15F805FE#A10708090A0B0C0D                          \n\
        (1739920494.582015) can0 15F805FE#A20E0F101112                              \n\
        (1739229594.475454) can0 10670CEC#7AF24B3B8DA6BE03                          \n\
        (1750992427.225496) can0 18ECF9A4#101F0005FFDAFE00	// TP.CM_RTS (A4 -> F9) \n\
        (1750992427.243729) can0 1CECA4F9#110501FFFFDAFE00	// TP.CM_CTS            \n\
        (1750992427.253501) can0 1CECA4F9#103B0009FFC5FD00	// TP.CM_RTS (F9 -> A4) \n\
        (1750992427.261216) can0 1CEBF9A4#0111111111111111	// TP.DT                \n\
        (1750992427.261791) can0 1CEBF9A4#0222222222222222	// TP.DT                \n\
        (1750992427.262356) can0 1CEBF9A4#0333333333333333	// TP.DT                \n\
        (1750992427.262911) can0 1CEBF9A4#0444444444444444	// TP.DT                \n\
        (1750992427.263480) can0 1CEBF9A4#05555555FFFFFFFF	// TP.DT                \n\
        (1750992427.266323) can0 1CECA4F9#131F0005FFDAFE00	// TP.CM_EndofMsgACK    \n\
        (1750992427.268593) can0 18ECF9A4#110901FFFFC5FD00	// TP.CM_CTS            \n\
        (1750992427.271783) can0 1CEBA4F9#0111111111111111	// TP.DT                \n\
        (1750992427.271819) can0 1CEBA4F9#0222222222222222	// TP.DT                \n\
        (1750992427.274755) can0 1CEBA4F9#0333333333333333	// TP.DT                \n\
        (1750992427.275845) can0 1CEBA4F9#0444444444444444	// TP.DT                \n\
        (1750992427.276926) can0 1CEBA4F9#0555555555555555	// TP.DT                \n\
        (1750992427.278029) can0 1CEBA4F9#0666666666666666	// TP.DT                \n\
        (1750992427.279146) can0 1CEBA4F9#0777777777777777	// TP.DT                \n\
        (1750992427.280212) can0 1CEBA4F9#0888888888888888	// TP.DT                \n\
        (1750992427.281361) can0 1CEBA4F9#09999999FFFFFFFF	// TP.DT                \n\
        (1750992427.295025) can0 18ECF9A4#133B0009FFC5FD00	// TP.CM_EndofMsgACK    \n\
    ";

    let expected = "\
        (1739920494.579828) can0 15F805FE#0102030405060708090A0B0C0D0E0F101112\n\
        (1739229594.475454) can0 10670CEC#7AF24B3B8DA6BE03\n\
        (1750992427.266323) can0 18FEDAA4#11111111111111222222222222223333333333333344444444444444555555\n\
        (1750992427.295025) can0 1CFDC5F9#1111111111111122222222222222333333333333334444444444444455555555555555666666666666667777777777777788888888888888999999\n\
    ";

    let output = tool!("canstruct")
        .write_stdin(input)
        .captured_output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout, expected);
}
