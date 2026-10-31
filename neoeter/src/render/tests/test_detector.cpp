#include <support/TestAssert.h>

#include <Neoeter/Render/StdAfx.h>
#include <Neoeter/Render/CollisionData.h>

#include <cmath>

namespace
{

struct ObbCase
{
    CBaseCollisionInstance * instance = nullptr;
    D3DXMATRIX world;
    D3DXVECTOR3 half;
};

ObbCase makeObb(float yawRadians, const D3DXVECTOR3 & centre, const D3DXVECTOR3 & halfExtents)
{
    CStaticCollisionData data = {};
    data.dwType = COLLISION_TYPE_OBB;
    data.v3Position = D3DXVECTOR3(0.0f, 0.0f, 0.0f);
    data.fDimensions[0] = halfExtents.x;
    data.fDimensions[1] = halfExtents.y;
    data.fDimensions[2] = halfExtents.z;
    D3DXQuaternionIdentity(&data.quatRotation);

    D3DXMATRIX rotation;
    D3DXMatrixRotationZ(&rotation, yawRadians);
    D3DXMATRIX translation;
    D3DXMatrixTranslation(&translation, centre.x, centre.y, centre.z);

    ObbCase out;
    out.world = rotation * translation;
    out.half = halfExtents;
    out.instance = CBaseCollisionInstance::BuildCollisionInstance(&data, &out.world);
    TEST_ASSERT(out.instance != nullptr);
    return out;
}

bool collides(CBaseCollisionInstance * instance, const D3DXVECTOR3 & worldPosition,
              float radius)
{
    CDynamicSphereInstance sphere = {};
    sphere.v3Position = worldPosition;
    sphere.v3LastPosition = worldPosition;
    sphere.fRadius = radius;
    return instance->CollisionDynamicSphere(sphere);
}

bool collidesMoving(CBaseCollisionInstance * instance, const D3DXVECTOR3 & current,
                    const D3DXVECTOR3 & previous, float radius)
{
    CDynamicSphereInstance sphere = {};
    sphere.v3Position = current;
    sphere.v3LastPosition = previous;
    sphere.fRadius = radius;
    return instance->MovementCollisionDynamicSphere(sphere);
}

D3DXVECTOR3 toWorld(const ObbCase & obb, const D3DXVECTOR3 & local)
{
    D3DXVECTOR3 world;
    D3DXVec3TransformCoord(&world, &local, &obb.world);
    return world;
}

void test_axis_aligned_obb()
{
    TEST_SECTION("OBB (axis aligned)");

    ObbCase obb = makeObb(0.0f, D3DXVECTOR3(5.0f, 5.0f, 0.0f), D3DXVECTOR3(2.0f, 0.5f, 0.5f));

    TEST_ASSERT(collides(obb.instance, toWorld(obb, D3DXVECTOR3(0.0f, 0.0f, 0.0f)), 0.01f));
    TEST_ASSERT(collides(obb.instance, toWorld(obb, D3DXVECTOR3(1.5f, 0.4f, 0.0f)), 0.01f));
    TEST_ASSERT(!collides(obb.instance, toWorld(obb, D3DXVECTOR3(2.5f, 0.0f, 0.0f)), 0.01f));

    // Sphere radius reaches the face: the closest point is 0.2 away, radius
    // 0.25 must collide, radius 0.1 must not.
    TEST_ASSERT(collides(obb.instance, toWorld(obb, D3DXVECTOR3(2.2f, 0.0f, 0.0f)), 0.25f));
    TEST_ASSERT(!collides(obb.instance, toWorld(obb, D3DXVECTOR3(2.2f, 0.0f, 0.0f)), 0.1f));

    // The movement variant also tests the previous position.
    TEST_ASSERT(collidesMoving(obb.instance, toWorld(obb, D3DXVECTOR3(3.0f, 0.0f, 0.0f)),
                               toWorld(obb, D3DXVECTOR3(0.0f, 0.0f, 0.0f)), 0.01f));
    TEST_ASSERT(!collidesMoving(obb.instance, toWorld(obb, D3DXVECTOR3(3.0f, 0.0f, 0.0f)),
                                toWorld(obb, D3DXVECTOR3(4.0f, 0.0f, 0.0f)), 0.01f));

    obb.instance->Destroy();
    TEST_PASS("inside, outside, radius margin and swept check");
}

void test_rotated_obb()
{
    TEST_SECTION("OBB (45 degree yaw)");

    // A long, thin box turned 45 degrees. The local point (1.5, 0.4) is inside
    // (|x| <= 2, |y| <= 0.5); after the transform it lands close to the rotated
    // long axis.
    ObbCase obb = makeObb(D3DX_PI / 4.0f, D3DXVECTOR3(5.0f, 5.0f, 0.0f),
                          D3DXVECTOR3(2.0f, 0.5f, 0.5f));

    const D3DXVECTOR3 insideLocal(1.5f, 0.4f, 0.0f);
    TEST_ASSERT(collides(obb.instance, toWorld(obb, insideLocal), 0.001f));

    const D3DXVECTOR3 outsideLocal(2.5f, 0.0f, 0.0f);
    TEST_ASSERT(!collides(obb.instance, toWorld(obb, outsideLocal), 0.001f));

    obb.instance->Destroy();
    TEST_PASS("local-space containment survives rotation");
}

} // namespace

int main()
{
    test_axis_aligned_obb();
    test_rotated_obb();
    DestroyCollisionInstanceSystem();

    std::printf("[test_detector] all checks passed\n");
    return 0;
}
