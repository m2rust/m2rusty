// Reference dump for crates/m2-net.
//
//   m2diff_net layout            S <struct> <sizeof>, F <struct> <field> <offsetof> <sizeof field>
//   m2diff_net headers           H <CG|GC> <name> <value>
//   m2diff_net map               M <header> <packet size> <dynamic 0|1> from the real
//                                CNetworkPacketHeaderMap filled by the Set(...) lines of
//                                CMainPacketHeaderMap (copied verbatim by gen_packets.pl)
//   m2diff_net sequence <n>      the first n bytes CNetworkStream::SendSequence puts on a real
//                                loopback socket (hex, one line)
//   m2diff_net frame <script>    runs scripted server bytes through the REAL CNetworkStream
//                                (loopback TCP, Process/Peek/Recv) and the CheckPacket logic of
//                                CPythonNetworkStream copied verbatim (see CheckPacketExtract)
//
// frame script lines: "chunk <hex>" (server sends these bytes, then the client polls and
// dispatches every complete packet) or "end". Output: "P <header> <len> <hex>" per packet,
// "U <header>" for an unknown header (C++: ClearRecvBuffer), "R <bytes left in recv buffer>"
// after each chunk.

#include <Client/UI/StdAfx.h>
#include <Neoeter/Render/NetPacketHeaderMap.h>
#include <Neoeter/Render/NetStream.h>
#include <Client/UI/Packet.h>

#include <arpa/inet.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <unistd.h>

#include <cstddef>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

namespace
{

void DumpLayout()
{
#define STRUCT(T) std::printf("S %s %zu\n", #T, sizeof(T));
#define FIELD(T, f) std::printf("F %s %s %zu %zu\n", #T, #f, offsetof(T, f), sizeof(((T *)nullptr)->f));
#define END_STRUCT
#define HDR(dir, n)
#define Set(...)
#include "layouts.inc"
#undef STRUCT
#undef FIELD
#undef END_STRUCT
#undef HDR
#undef Set
}

void DumpHeaders()
{
#define STRUCT(T)
#define FIELD(T, f)
#define END_STRUCT
#define HDR(dir, n) std::printf("H %s %s %u\n", #dir, #n, (unsigned)HEADER_##dir##_##n);
#define Set(...)
#include "layouts.inc"
#undef STRUCT
#undef FIELD
#undef END_STRUCT
#undef HDR
#undef Set
}

class CDumpHeaderMap : public CNetworkPacketHeaderMap
{
  public:
    enum
    {
        STATIC_SIZE_PACKET = false,
        DYNAMIC_SIZE_PACKET = true,
    };

    CDumpHeaderMap()
    {
#define STRUCT(T)
#define FIELD(T, f)
#define END_STRUCT
#define HDR(dir, n)
#include "layouts.inc"
#undef STRUCT
#undef FIELD
#undef END_STRUCT
#undef HDR
    }

    bool GetEntry(int header, TPacketType *t)
    {
        return Get(header, t);
    }
};

void DumpMap()
{
    CDumpHeaderMap map;
    for (int h = 0; h < 256; ++h)
    {
        CNetworkPacketHeaderMap::TPacketType t;
        if (map.GetEntry(h, &t))
            std::printf("M %d %d %d\n", h, t.iPacketSize, t.isDynamicSizePacket ? 1 : 0);
    }
}

// ---------------------------------------------------------------- loopback harness
struct Loopback
{
    int listener = -1;
    int server = -1;
    int port = 0;

    bool Listen()
    {
        listener = socket(AF_INET, SOCK_STREAM, 0);
        sockaddr_in a{};
        a.sin_family = AF_INET;
        a.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        a.sin_port = 0;
        if (bind(listener, reinterpret_cast<sockaddr *>(&a), sizeof(a)) != 0)
            return false;
        socklen_t len = sizeof(a);
        getsockname(listener, reinterpret_cast<sockaddr *>(&a), &len);
        port = ntohs(a.sin_port);
        return listen(listener, 1) == 0;
    }

    bool Accept()
    {
        server = accept(listener, nullptr, nullptr);
        return server >= 0;
    }
};

bool ConnectPair(CNetworkStream &stream, Loopback &lb)
{
    if (!lb.Listen())
        return false;
    stream.SetRecvBufferSize(1 << 20);
    stream.SetSendBufferSize(1 << 20);
    if (!stream.Connect("127.0.0.1", lb.port))
        return false;
    if (!lb.Accept())
        return false;
    for (int i = 0; i < 1000 && !stream.IsOnline(); ++i)
    {
        stream.Process();
        usleep(1000);
    }
    return stream.IsOnline();
}

std::vector<unsigned char> ParseHex(const std::string &s)
{
    std::vector<unsigned char> out;
    for (size_t i = 0; i + 1 < s.size(); i += 2)
        out.push_back(static_cast<unsigned char>(std::strtoul(s.substr(i, 2).c_str(), nullptr, 16)));
    return out;
}

std::string ToHex(const unsigned char *p, size_t n)
{
    static const char *d = "0123456789abcdef";
    std::string s;
    for (size_t i = 0; i < n; ++i)
    {
        s += d[p[i] >> 4];
        s += d[p[i] & 15];
    }
    return s;
}

void DumpSequence(int n)
{
    Loopback lb;
    CNetworkStream stream;
    if (!ConnectPair(stream, lb))
    {
        std::fprintf(stderr, "loopback failed\n");
        return;
    }
    stream.SetPacketSequenceMode(true);
    std::vector<unsigned char> got;
    int sent = 0;
    while (static_cast<int>(got.size()) < n)
    {
        // The send buffer is 1 MiB; stay far below it.
        for (int i = 0; i < 4096 && sent < n; ++i, ++sent)
            stream.SendSequence();
        for (int spin = 0; spin < 50; ++spin)
        {
            stream.Process();
            unsigned char buf[8192];
            ssize_t r = recv(lb.server, buf, sizeof(buf), MSG_DONTWAIT);
            if (r > 0)
                got.insert(got.end(), buf, buf + r);
            else
                usleep(500);
            if (static_cast<int>(got.size()) >= sent)
                break;
        }
    }
    std::printf("%s\n", ToHex(got.data(), static_cast<size_t>(n)).c_str());
}

// CPythonNetworkStream::CheckPacket (client/src/ui/PythonNetworkStream.cpp), with the unknown
// header branch reporting instead of PostQuitMessage and the trace calls removed.
bool CheckPacketExtract(CNetworkStream &s, CDumpHeaderMap &map, TPacketHeader *pRetHeader, bool *unknown)
{
    *pRetHeader = 0;
    *unknown = false;

    TPacketHeader header = 0;

    if (!s.Peek(sizeof(TPacketHeader), &header))
        return false;

    if (0 == header)
    {
        if (!s.Recv(sizeof(TPacketHeader), &header))
            return false;

        while (s.Peek(sizeof(TPacketHeader), &header))
        {
            if (0 == header)
            {
                if (!s.Recv(sizeof(TPacketHeader), &header))
                    return false;
            }
            else
            {
                break;
            }
        }

        if (0 == header)
            return false;
    }

    CNetworkPacketHeaderMap::TPacketType PacketType;

    if (!map.GetEntry(header, &PacketType))
    {
        std::printf("U %u\n", (unsigned)header);
        s.ClearRecvBuffer();
        *unknown = true;
        return false;
    }

    if (PacketType.isDynamicSizePacket)
    {
        TDynamicSizePacketHeader DynamicSizePacketHeader;

        if (!s.Peek(sizeof(TDynamicSizePacketHeader), &DynamicSizePacketHeader))
            return false;

        if (!s.Peek(DynamicSizePacketHeader.size))
            return false;
    }
    else
    {
        if (!s.Peek(PacketType.iPacketSize))
            return false;
    }

    if (!header)
        return false;

    *pRetHeader = header;
    return true;
}

void RunFrame(const char *scriptPath)
{
    Loopback lb;
    CNetworkStream stream;
    if (!ConnectPair(stream, lb))
    {
        std::fprintf(stderr, "loopback failed\n");
        return;
    }
    CDumpHeaderMap map;

    std::ifstream in(scriptPath);
    std::string line;
    while (std::getline(in, line))
    {
        std::istringstream ls(line);
        std::string cmd, hex;
        ls >> cmd >> hex;
        if (cmd != "chunk")
            continue;
        std::vector<unsigned char> bytes = ParseHex(hex);
        size_t off = 0;
        while (off < bytes.size())
        {
            ssize_t w = send(lb.server, bytes.data() + off, bytes.size() - off, 0);
            if (w <= 0)
                break;
            off += static_cast<size_t>(w);
        }
        // Poll until the stream has taken the bytes (headers may be consumed by
        // the dispatch below, so count what Process() adds to the buffer).
        int const before = stream.GetRecvBufferSize();
        for (int spin = 0; spin < 2000; ++spin)
        {
            stream.Process();
            if (stream.GetRecvBufferSize() - before >= static_cast<int>(bytes.size()))
                break;
            usleep(200);
        }
        TPacketHeader header = 0;
        bool unknown = false;
        while (CheckPacketExtract(stream, map, &header, &unknown))
        {
            CNetworkPacketHeaderMap::TPacketType t;
            map.GetEntry(header, &t);
            int size = t.iPacketSize;
            if (t.isDynamicSizePacket)
            {
                TDynamicSizePacketHeader d;
                stream.Peek(sizeof(d), &d);
                size = d.size;
            }
            std::vector<unsigned char> buf(static_cast<size_t>(size > 0 ? size : 1));
            stream.Recv(size, buf.data());
            std::printf("P %u %d %s\n", (unsigned)header, size, ToHex(buf.data(), static_cast<size_t>(size)).c_str());
        }
        std::printf("R %d\n", stream.GetRecvBufferSize());
    }
}

} // namespace

int main(int argc, char **argv)
{
    std::string mode = argc > 1 ? argv[1] : "";
    if (mode == "layout")
        DumpLayout();
    else if (mode == "headers")
        DumpHeaders();
    else if (mode == "map")
        DumpMap();
    else if (mode == "sequence" && argc > 2)
        DumpSequence(std::atoi(argv[2]));
    else if (mode == "frame" && argc > 2)
        RunFrame(argv[2]);
    else
    {
        std::fprintf(stderr, "usage: m2diff_net layout|headers|map|sequence <n>|frame <script>\n");
        return 2;
    }
    return 0;
}
