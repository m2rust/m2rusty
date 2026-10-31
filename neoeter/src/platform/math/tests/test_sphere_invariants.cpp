#include <support/TestAssert.h>

#include <Neoeter/Platform/Math/sphere.h>
#include <Neoeter/Platform/Math/vector.h>

#include <cmath>

namespace
{

bool near(float a, float b, float epsilon = 1e-4f)
{
    return std::fabs(a - b) <= epsilon;
}

} // namespace

int main()
{
    TEST_SECTION("Vector3d arithmetic");

    const Vector3d a(1.0f, 2.0f, 3.0f);
    const Vector3d b(4.0f, 5.0f, 6.0f);

    TEST_ASSERT((a + b) == Vector3d(5.0f, 7.0f, 9.0f));
    TEST_ASSERT((b - a) == Vector3d(3.0f, 3.0f, 3.0f));
    TEST_ASSERT((a * 2.0f) == Vector3d(2.0f, 4.0f, 6.0f));
    TEST_ASSERT((a / 2.0f) == Vector3d(0.5f, 1.0f, 1.5f));
    TEST_ASSERT((-a) == Vector3d(-1.0f, -2.0f, -3.0f));
    TEST_ASSERT(near(a.Dot(b), 32.0f));
    TEST_ASSERT(near(a.Length2(), 14.0f));
    TEST_ASSERT(near(a.Length(), std::sqrt(14.0f)));
    TEST_PASS("add/sub/scale/negate/dot/length");

    TEST_SECTION("Sphere invariants");

    Sphere sphere(Vector3d(0.0f, 0.0f, 10.0f), 2.0f);
    TEST_ASSERT(near(sphere.GetRadius(), 2.0f));
    TEST_ASSERT(near(sphere.GetRadius2(), 4.0f));

    sphere.SetRadius(3.0f);
    TEST_ASSERT(near(sphere.GetRadius(), 3.0f));
    TEST_ASSERT(near(sphere.GetRadius2(), 9.0f));
    TEST_ASSERT(sphere.GetCenter() == Vector3d(0.0f, 0.0f, 10.0f));
    TEST_PASS("SetRadius keeps radius^2 consistent");

    // InSphere uses a strict comparison against radius + margin.
    TEST_ASSERT(sphere.InSphere(Vector3d(0.0f, 0.0f, 13.0f), 0.5f));
    TEST_ASSERT(!sphere.InSphere(Vector3d(0.0f, 0.0f, 14.0f), 0.5f));
    TEST_PASS("InSphere margin");

    // InSphereXY ignores the z axis entirely.
    TEST_ASSERT(sphere.InSphereXY(Vector3d(0.0f, 0.0f, 1000.0f), 0.5f));
    TEST_ASSERT(!sphere.InSphereXY(Vector3d(100.0f, 0.0f, 10.0f), 0.5f));
    TEST_PASS("InSphereXY is z-independent");

    TEST_SECTION("Ray/sphere intersection");

    Vector3d hit;
    TEST_ASSERT(sphere.RayIntersectionInFront(Vector3d(0.0f, 0.0f, 0.0f), Vector3d(0.0f, 0.0f, 1.0f), &hit));
    TEST_ASSERT(near(hit.z, 7.0f));
    TEST_PASS("ray from -z hits the near surface");

    TEST_ASSERT(!sphere.RayIntersectionInFront(Vector3d(0.0f, 0.0f, 0.0f), Vector3d(0.0f, 0.0f, -1.0f), &hit));
    TEST_PASS("ray pointing away is rejected by InFront");

    // A ray starting inside reports the far surface.
    Vector3d insideHit;
    TEST_ASSERT(sphere.RayIntersection(Vector3d(0.0f, 0.0f, 10.0f), Vector3d(0.0f, 0.0f, 1.0f), &insideHit));
    TEST_ASSERT(near(insideHit.z, 13.0f));
    TEST_PASS("ray starting inside hits the far surface");

    // The distance-limited overload rejects hits beyond the limit.
    TEST_ASSERT(!sphere.RayIntersection(Vector3d(0.0f, 0.0f, 0.0f), Vector3d(0.0f, 0.0f, 1.0f), 5.0f, &hit));
    TEST_ASSERT(sphere.RayIntersection(Vector3d(0.0f, 0.0f, 0.0f), Vector3d(0.0f, 0.0f, 1.0f), 8.0f, &hit));
    TEST_PASS("distance limit");

    return 0;
}
