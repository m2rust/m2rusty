#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/ReferenceObject.h>

CReferenceObject::CReferenceObject()
{
}

CReferenceObject::~CReferenceObject() = default;

void CReferenceObject::AddReference()
{
    if (m_refCount == 0)
        OnConstruct();

    ++m_refCount;
}

int CReferenceObject::GetReferenceCount() const
{
    return m_refCount;
}

void CReferenceObject::AddReferenceOnly()
{
    ++m_refCount;
}

void CReferenceObject::Release()
{
    if (m_refCount > 1)
    {
        --m_refCount;
        return;
    }

    assert(m_destructed == false);
    assert(m_refCount >= 0);
    m_refCount = 0;
    OnSelfDestruct();
}

void CReferenceObject::OnConstruct()
{
    m_destructed = false;
}

void CReferenceObject::OnSelfDestruct()
{
    m_destructed = true;
    delete this;
}

bool CReferenceObject::canDestroy() const
{
    return m_refCount <= 0;
}
