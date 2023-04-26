/*
    Phaneron media compositing software.
    Original work Copyright (C) 2020 Streampunk Media Ltd.
    Based on work from [Streampunk Media Ltd.](https://github.com/Streampunk/phaneron)
    Further work Copyright (C) 2023 SuperFlyTV AB.

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

__kernel void read(
    __global uchar8* restrict inputY,
    __global uchar4* restrict inputU,
    __global uchar4* restrict inputV,
    __global float4* restrict output,
    __private unsigned int width,
    __constant float4* restrict colMatrix,
    __global float* restrict gammaLut,
    __constant float4* restrict gamutMatrix
) {
    bool lastItemOnLine = get_local_id(0) == get_local_size(0) - 1;

    // 64 output pixels per workItem = 8 input luma uchar8s per work item, 8 each u & v uchar4s per work item
    uint numPixels = lastItemOnLine && (0 != width % 64) ? width % 64 : 64;
    uint numLoops = numPixels / 8;
    uint remain = numPixels % 8;

    uint pitchReads = (width + 7) / 8;
    uint inOff = 8 * get_local_id(0) + pitchReads * get_group_id(0);
    uint outOff = width * get_group_id(0) + get_local_id(0) * 64;

    float4 colMatR = colMatrix[0];
    float4 colMatG = colMatrix[1];
    float4 colMatB = colMatrix[2];

    // optimise loading of the 3x3 gamut matrix
    float4 gamutMat0 = gamutMatrix[0];
    float4 gamutMat1 = gamutMatrix[1];
    float4 gamutMat2 = gamutMatrix[2];
    float3 gamutMatR = (float3)(gamutMat0.s0, gamutMat0.s1, gamutMat0.s2);
    float3 gamutMatG = (float3)(gamutMat0.s3, gamutMat1.s0, gamutMat1.s1);
    float3 gamutMatB = (float3)(gamutMat1.s2, gamutMat1.s3, gamutMat2.s0);

    for (uint i=0; i<numLoops; ++i) {
        uchar8 y = inputY[inOff];
        uchar4 u = inputU[inOff];
        uchar4 v = inputV[inOff];

        uchar4 yuva[8];
        yuva[0] = (uchar4)(y.s0, u.s0, v.s0, 1);
        yuva[1] = (uchar4)(y.s1, u.s0, v.s0, 1);
        yuva[2] = (uchar4)(y.s2, u.s1, v.s1, 1);
        yuva[3] = (uchar4)(y.s3, u.s1, v.s1, 1);
        yuva[4] = (uchar4)(y.s4, u.s2, v.s2, 1);
        yuva[5] = (uchar4)(y.s5, u.s2, v.s2, 1);
        yuva[6] = (uchar4)(y.s6, u.s3, v.s3, 1);
        yuva[7] = (uchar4)(y.s7, u.s3, v.s3, 1);

        for (uint p=0; p<8; ++p) {
            float4 yuva_f = convert_float4(yuva[p]);
            float3 rgb;
            rgb.s0 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatR) * 65535.0f)];
            rgb.s1 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatG) * 65535.0f)];
            rgb.s2 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatB) * 65535.0f)];

            float4 rgba;
            rgba.s0 = dot(rgb, gamutMatR);
            rgba.s1 = dot(rgb, gamutMatG);
            rgba.s2 = dot(rgb, gamutMatB);
            rgba.s3 = 1.0f;
            output[outOff+p] = rgba;
        }

        inOff++;
        outOff+=8;
    }

    if (remain > 0) {
        uchar8 y = inputY[inOff];
        uchar4 u = inputU[inOff];
        uchar4 v = inputV[inOff];

        uchar4 yuva[6];
        yuva[0] = (uchar4)(y.s0, u.s0, v.s0, 1);
        yuva[1] = (uchar4)(y.s1, u.s0, v.s0, 1);

        if (remain > 2) {
            yuva[2] = (uchar4)(y.s2, u.s1, v.s1, 1);
            yuva[3] = (uchar4)(y.s3, u.s1, v.s1, 1);

            if (remain > 4) {
                yuva[4] = (uchar4)(y.s4, u.s2, v.s2, 1);
                yuva[5] = (uchar4)(y.s5, u.s2, v.s2, 1);
            }
        }

        for (uint p=0; p<remain; ++p) {
            float4 yuva_f = convert_float4(yuva[p]);
            float3 rgb;
            rgb.s0 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatR) * 65535.0f)];
            rgb.s1 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatG) * 65535.0f)];
            rgb.s2 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatB) * 65535.0f)];

            float4 rgba;
            rgba.s0 = dot(rgb, gamutMatR);
            rgba.s1 = dot(rgb, gamutMatG);
            rgba.s2 = dot(rgb, gamutMatB);
            rgba.s3 = 1.0f;
            output[outOff+p] = rgba;
        }
    }
}
