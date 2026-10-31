#include <support/TestAssert.h>

#include <Client/UI/StdAfx.h>
#include <Neoeter/Render/NetPacketHeaderMap.h>
#include <Client/UI/Packet.h>

#include <cstddef>
#include <cstdio>
#include <cstring>
#include <vector>

namespace
{

template <typename T>
std::vector<unsigned char> Encode(const T & packet)
{
    std::vector<unsigned char> bytes(sizeof(T));
    std::memcpy(bytes.data(), &packet, sizeof(T));
    return bytes;
}

template <typename T>
bool Decode(const std::vector<unsigned char> & bytes, T & packet)
{
    if (bytes.size() < sizeof(T))
        return false;
    std::memcpy(&packet, bytes.data(), sizeof(T));
    return true;
}

void test_wire_sizes()
{
    TEST_SECTION("packet wire sizes");

    TEST_ASSERT(sizeof(TPacketHeader) == 1);
    TEST_ASSERT(sizeof(TPacketGCBlank) == 1);
    TEST_ASSERT(sizeof(TPacketGCPhase) == 2);
    TEST_ASSERT(sizeof(TPacketCGSelectCharacter) == 2);
    TEST_ASSERT(sizeof(TDynamicSizePacketHeader) == 3);
    TEST_ASSERT(sizeof(TPacketCGChat) == 4);
    TEST_ASSERT(sizeof(TPacketGCPosition) == 6);
    TEST_ASSERT(sizeof(TPacketCGAttack) == 8);
    TEST_ASSERT(sizeof(TPacketGCHandshake) == 13);
    TEST_ASSERT(sizeof(TPacketGCMove) == 24);

    TEST_PASS("structs are byte-packed with no tail padding");
}

void test_field_offsets()
{
    TEST_SECTION("packet field offsets");

    TEST_ASSERT(offsetof(TPacketGCHandshake, header) == 0);
    TEST_ASSERT(offsetof(TPacketGCHandshake, dwHandshake) == 1);
    TEST_ASSERT(offsetof(TPacketGCHandshake, dwTime) == 5);
    TEST_ASSERT(offsetof(TPacketGCHandshake, lDelta) == 9);

    TEST_ASSERT(offsetof(TPacketGCMove, bHeader) == 0);
    TEST_ASSERT(offsetof(TPacketGCMove, bFunc) == 1);
    TEST_ASSERT(offsetof(TPacketGCMove, bArg) == 2);
    TEST_ASSERT(offsetof(TPacketGCMove, bRot) == 3);
    TEST_ASSERT(offsetof(TPacketGCMove, dwVID) == 4);
    TEST_ASSERT(offsetof(TPacketGCMove, lX) == 8);
    TEST_ASSERT(offsetof(TPacketGCMove, lY) == 12);
    TEST_ASSERT(offsetof(TPacketGCMove, dwTime) == 16);
    TEST_ASSERT(offsetof(TPacketGCMove, dwDuration) == 20);

    TEST_PASS("multi-byte fields start at the on-wire offsets");
}

void test_round_trip()
{
    TEST_SECTION("packet (de)serialization");

    TPacketGCMove move = {};
    move.bHeader = HEADER_GC_CHARACTER_MOVE;
    move.bFunc = 7;
    move.bArg = 0xAB;
    move.bRot = 0x5A;
    move.dwVID = 0xDEADBEEFu;
    move.lX = -123456789;
    move.lY = 987654321;
    move.dwTime = 0x01020304u;
    move.dwDuration = 0xFFFFFFFEu;

    const std::vector<unsigned char> bytes = Encode(move);
    TEST_ASSERT(bytes.size() == sizeof(TPacketGCMove));
    TEST_ASSERT(bytes[0] == HEADER_GC_CHARACTER_MOVE);
    TEST_ASSERT(bytes[4] == 0xEF && bytes[5] == 0xBE && bytes[6] == 0xAD &&
                bytes[7] == 0xDE); // little-endian VID

    TPacketGCMove decoded = {};
    TEST_ASSERT(Decode(bytes, decoded));
    TEST_ASSERT(decoded.bHeader == move.bHeader);
    TEST_ASSERT(decoded.bFunc == move.bFunc);
    TEST_ASSERT(decoded.bArg == move.bArg);
    TEST_ASSERT(decoded.bRot == move.bRot);
    TEST_ASSERT(decoded.dwVID == move.dwVID);
    TEST_ASSERT(decoded.lX == move.lX);
    TEST_ASSERT(decoded.lY == move.lY);
    TEST_ASSERT(decoded.dwTime == move.dwTime);
    TEST_ASSERT(decoded.dwDuration == move.dwDuration);

    // A truncated buffer must be rejected before memcpy.
    std::vector<unsigned char> shortBytes(bytes.begin(), bytes.end() - 1);
    TPacketGCMove untouched = {};
    TEST_ASSERT(!Decode(shortBytes, untouched));
    TEST_ASSERT(untouched.bHeader == 0);

    TPacketGCHandshake handshake = {};
    handshake.header = HEADER_GC_HANDSHAKE;
    handshake.dwHandshake = 0x11223344u;
    handshake.dwTime = 0xAABBCCDDu;
    handshake.lDelta = -42;

    std::vector<unsigned char> handshakeBytes = Encode(handshake);
    TEST_ASSERT(handshakeBytes.size() == 13);
    TPacketGCHandshake decodedHandshake = {};
    TEST_ASSERT(Decode(handshakeBytes, decodedHandshake));
    TEST_ASSERT(decodedHandshake.dwHandshake == handshake.dwHandshake);
    TEST_ASSERT(decodedHandshake.dwTime == handshake.dwTime);
    TEST_ASSERT(decodedHandshake.lDelta == handshake.lDelta);

    TEST_PASS("fields survive encode/decode and short buffers are refused");
}

void test_header_map()
{
    TEST_SECTION("packet header map");

    CNetworkPacketHeaderMap map;

    CNetworkPacketHeaderMap::TPacketType type;
    TEST_ASSERT(!map.Get(HEADER_GC_PHASE, &type));

    map.Set(HEADER_GC_PHASE,
            CNetworkPacketHeaderMap::TPacketType(sizeof(TPacketGCPhase), false));
    map.Set(HEADER_GC_CHAT,
            CNetworkPacketHeaderMap::TPacketType(sizeof(TPacketGCChat), true));

    TEST_ASSERT(map.Get(HEADER_GC_PHASE, &type));
    TEST_ASSERT(type.iPacketSize == static_cast<int>(sizeof(TPacketGCPhase)));
    TEST_ASSERT(!type.isDynamicSizePacket);

    TEST_ASSERT(map.Get(HEADER_GC_CHAT, &type));
    TEST_ASSERT(type.iPacketSize == static_cast<int>(sizeof(TPacketGCChat)));
    TEST_ASSERT(type.isDynamicSizePacket);

    // Overwrite is allowed and replaces the descriptor.
    map.Set(HEADER_GC_PHASE, CNetworkPacketHeaderMap::TPacketType(99, true));
    TEST_ASSERT(map.Get(HEADER_GC_PHASE, &type));
    TEST_ASSERT(type.iPacketSize == 99);
    TEST_ASSERT(type.isDynamicSizePacket);

    TEST_ASSERT(!map.Get(HEADER_CG_LOGIN, &type));

    TEST_PASS("static/dynamic descriptors and unknown headers");
}

} // namespace

int main()
{
    test_wire_sizes();
    test_field_offsets();
    test_round_trip();
    test_header_map();

    std::printf("[test_packet] all checks passed\n");
    return 0;
}
