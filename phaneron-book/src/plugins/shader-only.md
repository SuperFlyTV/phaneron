# Shader-only Plugins

Shader-only plugins are the easiest way to extend the functionality of Phaneron as they only require a shader and a JSON file that describes the shader. Each shader will then be turned into an available node type, where the node type used to create a node from a shader named `my_shader.cl` would be `my_shader`.

For development, shaders live in the `phaneron-plugin-shaders` directory by default. In production, these shaders will be loaded from the `plugins` directory by default. Both of these can be changed using the `SHADER_PLUGINS_DIR` environment variable.

To begin, create two files: `my_shader.cl` and `my_shader.json`. The file names **must** match and must have the extensions `.cl` and `.json`, files such as `my_shader.description.json` **will not work**.

Inside `my_shader.cl` you can write a simple shader, for example, here is a shader that can flip images horizontally and/or vertically:

```c
__constant sampler_t sampler1 =
    CLK_NORMALIZED_COORDS_FALSE
    | CLK_ADDRESS_CLAMP_TO_EDGE
    | CLK_FILTER_NEAREST;

__kernel void flip(
    __read_only image2d_t input,
    __private char flip_horizontal,
    __private char flip_vertical,
    __write_only image2d_t output
) {
    int w = get_image_width(input);
    int h = get_image_height(input);

    int x = get_global_id(0);
    int sample_x = get_global_id(0);
    if (flip_horizontal > 0) {
        sample_x = w - x - 1;
    }
    int y = get_global_id(1);
    int sample_y = get_global_id(1);
    if (flip_vertical > 0) {
        sample_y = h - y - 1;
    }

    float4 out = read_imagef(input, sampler1, (int2)(sample_x, sample_y));
    write_imagef(output, (int2)(x, y), out);
}
```

Note: We use `char` to represent a bool. A value of `false` will be exactly `0_u8` and a value of `true` will be exactly `1_u8`.

Then, in your `my_shader.json` file, you must add a description of your shader:

```json
{
    "name": "Image Flip",
    "programName": "flip",
    "args": [
        {
            "type": "videoInput",
            "displayName": "Input"
        },
        {
            "type": "bool",
            "key": "flipHorizontal",
            "displayName": "Flip Horizontal",
            "defaultVal": false
        },
        {
            "type": "bool",
            "key": "flipVertical",
            "displayName": "Flip Vertical",
            "defaultVal": false
        },
        {
            "type": "videoOutput",
            "displayName": "Output"
        }
    ]
}
```

Where:
- `name` is the display name presented to UIs.
- `programName` is the name of the kernel program (after `__kernel void`)
- `args` is a non-empty array that describes the arguments to your shader in-order.

This results in a node type with the Id `flip` which can be used to create nodes. The state of these nodes can then be modified by passing a JSON object to Phaneron's state API, for example:

```json
{
    "flipHorizontal": true,
    "flipVertical": true
}
```

will flip the image upside-down and backwards.

## Args

The `args` array in the shader description must specify the arguments to your shader in the order that they should be passed to the shader. It must be a non-empty array, with the minimum required to be valid being an array containing a single video output.

For all arguments, a `displayName` is required to allow the argument to be displayed in UIs, it is also used for debugging.
For some arguments, a `key` is used to identify the value for that argument in the state object for the node created from the shader. This must be a valid JSON object key.

### Video Input

A video input is a video frame passed to your shader. All video inputs and outputs will be the same size.

```json
{
    "type": "videoInput",
    "displayName": "Input"
}
```

### Video Output

This is the output of your shader, you can have as many as you want but must have at least one.

```json
{
    "type": "videoOutput",
    "displayName": "Output"
}
```

### F32

This is a 32-bit float that will always be a value between 0.0 and 1.0 (inclusive). If a value is provided by the state that is outside of these bounds then the default value will be clamped to these bounds. The default value must be between 0.0 and 1.0 (inclusive) and is used when no value is provided in the state.


```json
{
    "type": "f32",
    "key": "transitionValue",
    "displayName": "Transition Value",
    "defaultValue": 0.0
}
```

### U32

This is a 32-bit unsigned integer. If a value is provided by the state that is outside of the `inclusiveMinimum` and `inclusiveMaximum` values then the value is clamped to these bounds. `inclusiveMaximum` is optional. The default value must be between `inclusiveMinimum` and `inclusiveMaximum` and is used when no value is provided in the state. `inclusiveMaximum` must be greater than `inclusiveMinimum`.

```json
{
    "type": "u32",
    "key": "currentStep",
    "displayName": "Current Step",
    "defaultValue": 100,
    "inclusiveMinimum": 50,
    "inclusiveMaximum": 200
}
```

### Bool

This is a boolean type, which will be passed to the shader as a char with `0_u8` for `false` and `1_u8` for `true`.

```json
{
    "type": "bool",
    "key": "flipHorizontal",
    "displayName": "Flip Horizontal",
    "defaultVal": false
}
```
