#include <Client/UI/StdAfx.h>
#include <Client/UI/PythonNetworkStream.h>
#include <Client/UI/PythonApplication.h>
#include <Client/UI/Packet.h>

void CPythonNetworkStream::OffLinePhase()
{
	TPacketHeader header;

	if (!CheckPacket(&header))
		return;

	switch (header)
	{
		case HEADER_GC_PHASE:
			if (RecvPhasePacket())
				return;
			break;
	}

	RecvErrorPacket(header);
}