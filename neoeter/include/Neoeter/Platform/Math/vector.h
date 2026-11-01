/* Copyright (C) John W. Ratcliff, 2001.
 * All rights reserved worldwide.
 *
 * This software is provided "as is" without express or implied
 * warranties. You may freely copy and compile this source into
 * applications you distribute provided that the copyright text
 * below is included in the resulting source code, for example:
 * "Portions Copyright (C) John W. Ratcliff, 2001"
 */
#pragma once

#include "StdAfx.h"
#include <math.h>
#include <vector>
// #include "stl.h"

/***********************************************************************/
/** VECTOR.H    : Template class to represent a 2d and 3d vector type. */
/**                                                                    */
/**               Written by John W. Ratcliff jratcliff@att.net        */
/***********************************************************************/

class Vector3d : public D3DXVECTOR3
{
  public:
    Vector3d() = default; // null constructor, does not inialize point.

    Vector3d(const Vector3d &a) : D3DXVECTOR3(a)
    {
        x = a.x;
        y = a.y;
        z = a.z;
    };

    Vector3d(float a, float b, float c) // construct with initial point.
    {
        x = a;
        y = b;
        z = c;
    };

    Vector3d *operator&()
    {
        return this;
    }
    const Vector3d *operator&() const
    {
        return this;
    }

    bool operator==(const Vector3d &a) const
    {
        return a.x == x && a.y == y && a.z == z;
    };

    bool operator!=(const Vector3d &a) const
    {
        return a.x != x || a.y != y || a.z != z;
    };

    // Operators
    Vector3d &operator=(const Vector3d &A) // ASSIGNMENT (=)
    {
        x = A.x;
        y = A.y;
        z = A.z;
        return (*this);
    };

    Vector3d operator+(const Vector3d &A) const // ADDITION (+)
    {
        Vector3d const Sum(x + A.x, y + A.y, z + A.z);
        return (Sum);
    };

    Vector3d operator-(const Vector3d &A) const // SUBTRACTION (-)
    {
        Vector3d const Diff(x - A.x, y - A.y, z - A.z);
        return (Diff);
    };

    Vector3d operator*(const float s) const // MULTIPLY BY SCALAR (*)
    {
        Vector3d const Scaled(x * s, y * s, z * s);
        return (Scaled);
    };

    Vector3d operator/(const float s) const // DIVIDE BY SCALAR (/)
    {
        float const r = 1.0f / s;
        Vector3d const Scaled(x * r, y * r, z * r);
        return (Scaled);
    };

    void operator+=(const Vector3d &A) // ACCUMULATED VECTOR ADDITION (+=)
    {
        x += A.x;
        y += A.y;
        z += A.z;
    };
    void operator-=(const Vector3d &A) // ACCUMULATED VECTOR SUBTRACTION (+=)
    {
        x -= A.x;
        y -= A.y;
        z -= A.z;
    };
    void operator*=(const float s) // ACCUMULATED SCALAR MULTIPLICATION (*=) (bpc 4/24/2000)
    {
        x *= s;
        y *= s;
        z *= s;
    }

    Vector3d operator-() const // NEGATION (-)
    {
        Vector3d const Negated(-x, -y, -z);
        return (Negated);
    };

    float operator[](const int i) const // ALLOWS VECTOR ACCESS AS AN ARRAY.
    {
        return ((i == 0) ? x : ((i == 1) ? y : z));
    };
    float &operator[](const int i)
    {
        return ((i == 0) ? x : ((i == 1) ? y : z));
    };
    //

    // accessor methods.
    float GetX() const
    {
        return x;
    };
    float GetY() const
    {
        return y;
    };
    float GetZ() const
    {
        return z;
    };

    void SetX(float t)
    {
        x = t;
    };
    void SetY(float t)
    {
        y = t;
    };
    void SetZ(float t)
    {
        z = t;
    };

    void Set(float a, float b, float c)
    {
        x = a;
        y = b;
        z = c;
    };

    void Zero()
    {
        x = y = z = 0;
    };

    // return -(*this).
    Vector3d negative() const
    {
        Vector3d result;
        result.x = -x;
        result.y = -y;
        result.z = -z;
        return result;
    }

    float Magnitude() const
    {
        return (sqrtf((x * x) + (y * y) + (z * z)));
    };

    void Lerp(const Vector3d &from, const Vector3d &to, float slerp)
    {
        *this = to - from; // delta on all 3 axis
        *this *= slerp;    // times interpolant distance.
        *this += from;     // plus source
    };

    float Length() const // length of vector.
    {
        return sqrtf((x * x) + (y * y) + (z * z));
    };

    float Length2() const // squared distance, prior to square root.
    {
        float const l2 = (x * x) + (y * y) + (z * z);
        return l2;
    };

    float Distance(const Vector3d &a) const // distance between two points.
    {
        return sqrtf(DistanceSq(a));
    }

    float Distance2d(const Vector3d &a) const // distance between two points.
    {
        return sqrtf(DistanceSq2d(a));
    }

    float DistanceXY(const Vector3d &a) const
    {
        float const dx = a.x - x;
        float const dy = a.y - y;
        float const dist = (dx * dx) + (dy * dy);
        return dist;
    }

    float DistanceSq(const Vector3d &a) const // squared distance.
    {
        float const dx = a.x - x;
        float const dy = a.y - y;
        float const dz = a.z - z;
        return (dx * dx) + (dy * dy) + (dz * dz);
    };

    float DistanceSq2d(const Vector3d &a) const // squared distance.
    {
        float const dx = a.x - x;
        float const dy = a.y - y;
        return (dx * dx) + (dy * dy);
    };

    float Normalize() // normalize to a unit vector, returns distance.
    {
        float const l = Length(); // get length.
        if (l != 0)
        {
            x /= l;
            y /= l;
            z /= l;
        }
        else
        {
            x = y = z = 0;
        }
        return l;
    };

    float Dot(const Vector3d &a) const // computes dot product.
    {
        return ((x * a.x) + (y * a.y) + (z * a.z));
    };
    bool IsInStaticRange() const;
    void Cross(const Vector3d &a,
               const Vector3d &b) // cross two vectors result in this one.
    {
        x = (a.y * b.z) - (a.z * b.y);
        y = (a.z * b.x) - (a.x * b.z);
        z = (a.x * b.y) - (a.y * b.x);
    };

    // private:

    //  float x;
    //  float y;
    //  float z;
};

using Vector3dVector = std::vector<Vector3d>;

inline Vector3d operator*(float s, const Vector3d &v)
{
    Vector3d const Scaled(v.x * s, v.y * s, v.z * s);
    return (Scaled);
};
