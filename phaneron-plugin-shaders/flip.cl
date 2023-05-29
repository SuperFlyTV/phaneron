/*
    Phaneron media compositing software.
    Copyright (C) 2023 SuperFlyTV AB.

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

__kernel void flip(
    __read_only image2d_t input,
    // __private char flip_vertical,
    // __private char flip_horizontal,
    __write_only image2d_t output
) {
    int x = get_global_id(0);
    int y = get_global_id(1);

    float4 out = read_imagef(input, sampler1, (int2)(x,y));
    write_imagef(output, (int2)(x, y), out);
}
