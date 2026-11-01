#include <Client/UI/StdAfx.h>
#include <Neoeter/Render/NetPacketHeaderMap.h>
#include <Neoeter/Render/NetStream.h>
#include <Client/UI/Packet.h>
#include <cstdio>
int main() { std::printf("%zu\n", sizeof(TPacketGCMove)); CNetworkStream s; return s.IsOnline(); }
