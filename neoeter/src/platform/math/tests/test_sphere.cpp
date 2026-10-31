#include <Neoeter/Platform/Math/StdAfx.h>
#include <Neoeter/Platform/Math/vector.h>
#include <Neoeter/Platform/Math/sphere.h>
#include <Neoeter/Platform/Math/frustum.h>
#include <Neoeter/Platform/Math/spherepack.h>

#include <cassert>
#include <cmath>
#include <cstdio>

struct TestCallback : public SpherePackCallback
{
    int rayHits = 0;
    int rangeHits = 0;

    void RayTraceCallback(
        const Vector3d& /*p1*/,
        const Vector3d& /*dir*/,
        float /*distance*/,
        const Vector3d& /*sect*/,
        SpherePack* /*sphere*/
    ) override
    {
        rayHits++;
    }

    void RangeTestCallback(
        const Vector3d& /*searchpos*/,
        float /*distance*/,
        SpherePack* /*sphere*/,
        ViewState /*state*/
    ) override
    {
        rangeHits++;
    }
};

int main()
{
    printf("[test_sphere] Running SphereLib unit tests...\n");

    // 1. Test Vector3d math
    {
        Vector3d a(3.0f, 4.0f, 0.0f);
        assert(std::fabs(a.Length() - 5.0f) < 1e-5f);
        assert(a.IsInStaticRange());

        Vector3d b(1.0f, 0.0f, 0.0f);
        assert(std::fabs(a.Dot(b) - 3.0f) < 1e-5f);
        printf("  [PASS] Vector3d length, dot product, and static range\n");
    }

    // 2. Test Sphere ray intersection & point containment
    {
        Sphere s(Vector3d(0.0f, 0.0f, 10.0f), 2.0f);
        assert(s.InSphere(Vector3d(0.0f, 0.0f, 9.0f), 0.5f));
        assert(!s.InSphere(Vector3d(0.0f, 0.0f, 20.0f), 0.5f));

        Vector3d hitPoint;
        bool hit = s.RayIntersectionInFront(Vector3d(0.0f, 0.0f, 0.0f), Vector3d(0.0f, 0.0f, 1.0f), &hitPoint);
        assert(hit);
        assert(std::fabs(hitPoint.z - 8.0f) < 1e-4f);
        printf("  [PASS] Sphere containment & ray-sphere intersection\n");
    }

    // 3. Test SpherePackFactory hierarchy, Process(), and RangeTest
    {
        SpherePackFactory factory(64, 1024.0f, 256.0f, 16.0f);
        SpherePack* sp1 = factory.AddSphere_(Vector3d(10.0f, 10.0f, 10.0f), 5.0f, nullptr, true);
        SpherePack* sp2 = factory.AddSphere_(Vector3d(500.0f, 500.0f, 500.0f), 5.0f, nullptr, true);
        assert(sp1 != nullptr);
        assert(sp2 != nullptr);

        factory.Process();

        TestCallback cb;
        factory.RangeTest(Vector3d(10.0f, 10.0f, 10.0f), 20.0f, &cb);
        assert(cb.rangeHits == 1);
        printf("  [PASS] SpherePackFactory spatial hierarchy & range query\n");
    }

    printf("[test_sphere] All tests passed successfully!\n");
    return 0;
}
