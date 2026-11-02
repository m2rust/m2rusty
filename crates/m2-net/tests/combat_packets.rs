//! The combat packets `m2-game` sends, byte for byte.

use m2_net::phase::{NetConfig, NetMachine};
use m2_net::wire::cg;

#[test]
fn attack_use_skill_shoot_target_and_fly_targeting() {
    let mut net = NetMachine::new(NetConfig::default());
    assert!(net.send_attack(0, 0x11223344));
    assert!(net.send_use_skill(34, 0x55667788));
    assert!(net.send_shoot(0));
    assert!(net.send_target(0x99aabbcc));
    assert!(net.send_fly_targeting(0x01020304, 1000, -2000));
    assert!(net.send_add_fly_targeting(0x05060708, 300, 400));
    assert!(net.send_guild_use_skill(151, 0));
    let bytes = net.take_output(4096);

    let expected = [
        // CG_ATTACK: header, type, victim, two CRC pieces.
        cg::ATTACK, 0, 0x44, 0x33, 0x22, 0x11, 0, 0,
        // CG_USE_SKILL: header, vnum, target.
        cg::USE_SKILL, 34, 0, 0, 0, 0x88, 0x77, 0x66, 0x55,
        // CG_SHOOT: header, type.
        cg::SHOOT, 0,
        // CG_TARGET: header, vid.
        cg::TARGET, 0xcc, 0xbb, 0xaa, 0x99,
        // CG_FLY_TARGETING: header, vid, x, y.
        cg::FLY_TARGETING, 0x04, 0x03, 0x02, 0x01, 0xe8, 0x03, 0, 0, 0x30, 0xf8, 0xff, 0xff,
        // CG_ADD_FLY_TARGETING: header, vid, x, y.
        cg::ADD_FLY_TARGETING, 0x08, 0x07, 0x06, 0x05, 0x2c, 0x01, 0, 0, 0x90, 0x01, 0, 0,
        // CG_GUILD: header, subheader 9, skill, target.
        cg::GUILD, 9, 151, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert_eq!(bytes, expected);
}
