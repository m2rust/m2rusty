#pragma once

// Collision Detection
using TSphereData = struct SSphereData
{
    D3DXVECTOR3 v3Position;
    float fRadius = 0.0F;
};

using TPlaneData = struct SPlaneData
{
    D3DXVECTOR3 v3Position;
    D3DXVECTOR3 v3Normal;

    D3DXVECTOR3 v3QuadPosition[4];
    D3DXVECTOR3 v3InsideVector[4];
};

using TAABBData = struct SAABBData
{
    D3DXVECTOR3 v3Min;
    D3DXVECTOR3 v3Max;
};

using TOBBData = struct SOBBData
{
    D3DXVECTOR3 v3Min;
    D3DXVECTOR3 v3Max;
    D3DXMATRIX matRot;
    D3DXMATRIX matRotInv; // world -> box local, used by the collision tests
};

using TCylinderData = struct SCylinderData
{
    D3DXVECTOR3 v3Position;
    float fRadius = 0.0F;
    float fHeight = 0.0F;
};

enum ECollisionType
{
    COLLISION_TYPE_PLANE,
    COLLISION_TYPE_BOX,
    COLLISION_TYPE_SPHERE,
    COLLISION_TYPE_CYLINDER,
    COLLISION_TYPE_AABB,
    COLLISION_TYPE_OBB,
};

struct CDynamicSphereInstance
{
    D3DXVECTOR3 v3Position;
    D3DXVECTOR3 v3LastPosition;

    float fRadius = 0.0F;
};

class CStaticCollisionData
{
  public:
    DWORD dwType = 0U;
    char szName[32 + 1]{};

    D3DXVECTOR3 v3Position;
    float fDimensions[3]{};
    D3DXQUATERNION quatRotation;
};

void DestroyCollisionInstanceSystem();

using CStaticCollisionDataVector = std::vector<CStaticCollisionData>;

/////////////////////////////////////////////
// Base
class CBaseCollisionInstance
{
  public:
    virtual ~CBaseCollisionInstance() = default;
    virtual void Render(D3DFILLMODE d3dFillMode = D3DFILL_SOLID) = 0;

    bool MovementCollisionDynamicSphere(const CDynamicSphereInstance &s) const
    {
        return OnMovementCollisionDynamicSphere(s);
    }
    bool CollisionDynamicSphere(const CDynamicSphereInstance &s) const
    {
        return OnCollisionDynamicSphere(s);
    }

    D3DXVECTOR3 GetCollisionMovementAdjust(const CDynamicSphereInstance &s) const
    {
        return OnGetCollisionMovementAdjust(s);
    }

    void Destroy();

    static CBaseCollisionInstance *BuildCollisionInstance(const CStaticCollisionData *c_pCollisionData,
                                                          const D3DXMATRIX *pMat);

  protected:
    virtual D3DXVECTOR3 OnGetCollisionMovementAdjust(const CDynamicSphereInstance &s) const = 0;
    virtual bool OnMovementCollisionDynamicSphere(const CDynamicSphereInstance &s) const = 0;
    virtual bool OnCollisionDynamicSphere(const CDynamicSphereInstance &s) const = 0;
    virtual void OnDestroy() = 0;
};

/////////////////////////////////////////////
// Sphere
class CSphereCollisionInstance : public CBaseCollisionInstance
{
  public:
    virtual ~CSphereCollisionInstance() = default;
    TSphereData &GetAttribute();
    const TSphereData &GetAttribute() const;
    void Render(D3DFILLMODE d3dFillMode = D3DFILL_SOLID) override;

  protected:
    void OnDestroy() override;
    bool OnMovementCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    bool OnCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    D3DXVECTOR3 OnGetCollisionMovementAdjust(const CDynamicSphereInstance &s) const override;

    TSphereData m_attribute;
};

/////////////////////////////////////////////
// Plane
class CPlaneCollisionInstance : public CBaseCollisionInstance
{
  public:
    virtual ~CPlaneCollisionInstance() = default;
    TPlaneData &GetAttribute();
    const TPlaneData &GetAttribute() const;
    void Render(D3DFILLMODE d3dFillMode = D3DFILL_SOLID) override;

  protected:
    void OnDestroy() override;
    bool OnMovementCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    bool OnCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    D3DXVECTOR3 OnGetCollisionMovementAdjust(const CDynamicSphereInstance &s) const override;

    TPlaneData m_attribute;
};

/////////////////////////////////////////////
// AABB (Aligned Axis Bounding Box)
class CAABBCollisionInstance : public CBaseCollisionInstance
{
  public:
    virtual ~CAABBCollisionInstance() = default;
    TAABBData &GetAttribute();
    const TAABBData &GetAttribute() const;
    void Render(D3DFILLMODE d3dFillMode = D3DFILL_SOLID) override;

  protected:
    void OnDestroy() override;
    bool OnMovementCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    bool OnCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    D3DXVECTOR3 OnGetCollisionMovementAdjust(const CDynamicSphereInstance &s) const override;

    TAABBData m_attribute;
};

/////////////////////////////////////////////
// OBB
class COBBCollisionInstance : public CBaseCollisionInstance
{
  public:
    virtual ~COBBCollisionInstance() = default;
    TOBBData &GetAttribute();
    const TOBBData &GetAttribute() const;
    void Render(D3DFILLMODE d3dFillMode = D3DFILL_SOLID) override;

  protected:
    void OnDestroy() override;
    bool OnMovementCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    bool OnCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    D3DXVECTOR3 OnGetCollisionMovementAdjust(const CDynamicSphereInstance &s) const override;

    TOBBData m_attribute;
};

/////////////////////////////////////////////
// Cylinder
class CCylinderCollisionInstance : public CBaseCollisionInstance
{
  public:
    virtual ~CCylinderCollisionInstance() = default;
    TCylinderData &GetAttribute();
    const TCylinderData &GetAttribute() const;
    void Render(D3DFILLMODE d3dFillMode = D3DFILL_SOLID) override;

  protected:
    void OnDestroy() override;
    bool OnMovementCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    bool OnCollisionDynamicSphere(const CDynamicSphereInstance &s) const override;
    D3DXVECTOR3 OnGetCollisionMovementAdjust(const CDynamicSphereInstance &s) const override;

    bool CollideCylinderVSDynamicSphere(const TCylinderData &c_rattribute, const CDynamicSphereInstance &s) const;

    TCylinderData m_attribute;
};

using CSphereCollisionInstanceVector = std::vector<CSphereCollisionInstance>;
using CDynamicSphereInstanceVector = std::vector<CDynamicSphereInstance>;
using CCollisionInstanceVector = std::vector<CBaseCollisionInstance *>;
