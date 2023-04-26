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

__constant sampler_t sampler1 =
    CLK_NORMALIZED_COORDS_FALSE
    | CLK_ADDRESS_CLAMP_TO_EDGE
    | CLK_FILTER_NEAREST;

__kernel void transition_dissolve(
    __read_only image2d_t input0,
    __read_only image2d_t input1,
    __private float mix,
    __write_only image2d_t output
) {
    int x = get_global_id(0);
    int y = get_global_id(1);
    float4 in0 = read_imagef(input0, sampler1, (int2)(x,y));
    float4 in1 = read_imagef(input1, sampler1, (int2)(x,y));
    float4 mix4 = (float4)(mix, mix, mix, mix);
    float rmix = 1.0f - mix;

    float4 out = fma(in0, mix4, in1 * rmix);
    write_imagef(output, (int2)(x, y), out);
}
