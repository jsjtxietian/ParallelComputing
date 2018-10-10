#include <smmintrin.h> // For _mm_stream_load_si128
#include <emmintrin.h> // For _mm_mul_ps
#include <assert.h>
#include <stdint.h>
#include <iostream>

using namespace std;

extern void saxpySerial(int N,
                        float scale,
                        float X[],
                        float Y[],
                        float result[]);

void print128_num(__m128 var)
{
    float *val = (float *)&var;
    cout << val[0] << " " << val[1] << " "
         << val[2] << " " << val[3] << " " << endl;
}

void saxpyStreaming(int N,
                    float scale,
                    float X[],
                    float Y[],
                    float result[])
{
    // Replace this code with ones that make use of the streaming instructions

    int loopTime = N / 4;

    __m128i m1,m4;
    __m128i *pX = (__m128i *)X;
    __m128i *pY = (__m128i *)Y;
    __m128i *pRe = (__m128i *)result;

    for (int i = 0; i < loopTime; i++)
    {
        m1 = _mm_stream_load_si128(pX);
        __m128 fm1 = _mm_castsi128_ps(m1);

        __m128 fscale = _mm_set_ps1(scale);
        __m128 first = _mm_mul_ps(fm1, fscale);

        m4 = _mm_stream_load_si128(pY);
        __m128 fm4 = _mm_castsi128_ps(m4);

        __m128 fm5 = _mm_add_ps(first, fm4);

        _mm_stream_si128(pRe,(__m128i)fm5);

        pX++;
        pY++;
        pRe++;
    }

}
