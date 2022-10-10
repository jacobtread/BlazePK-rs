# ⚙️BlazePK

![License](https://img.shields.io/github/license/jacobtread/BlazePk-rs?style=for-the-badge)

Rust library for working with the Blaze packet system this is the networking solution used by games such as
Mass Effect 3, Battlefield 3, another Other EA games. 

This is created for use with the Rust re-write of the [PocketRelay (https://github.com/jacobtread/PocketRelay)](https://github.com/jacobtread/PocketRelay) 
software.


### Creating decodable structs
In order to read the contents of packets you will need to define structs that 
the Tdf values can be mapped to.

> 🚩 **IMPORTANT** 🚩 When defining Tdf structs you must define the tag names in the 
> order in which they appear in the encoded packet. They are decoded by skipping
> values until the correct tag is reached so placing them in the incorrect order
> will result in them not being read

> All tag names must be defined in uppercase because reading and writing is case 
> sensitive.

#### Possible Types

When selecting types for packet fields you can only use types which inherit the Codec trait
the following table lists the following

> Any* = Any Other Mentioned Types

> A,B,C = Multiple types accepted

> The types u8 - u64 and i8 - i64 are cast to u64 and encoded using the VarInt encoding


| Type                                 | Details                               |
|--------------------------------------|---------------------------------------|
| u8, u16, u32, u64, i8, i16, i32, i64 | Converted to VarInt                   |
| VarInt                               | Variable length integer value         |
| String                               | Text encoded will a null terminator   |
| Vec\<u8>                             | Blob of bytes                         |
| Group                                | Group created with the group!() macro |
| Vec<String,VarInt,Float,Group>       | List of values                        |
| TdfMap<String,VarInt, Any*>          | Map of keys to values                 |
| TdfOptional<Any*>                    | Tdf value where value could be absent |
| VarIntList                           | List of variable length integers      |
| (VarInt, VarInt)                     | Pair of two VarInts                   |
| (VarInt, VarInt, VarInt)             | Tuple of three VarInts                |


```rust
use blaze_pk::packet;

packet! {
    struct MyPacket {
        TEST: u32,
        STR: String,
        BLOB: Vec<u8>
    }
}
```

### Creating nested structs

When you need to create a structure that is stored inside a packet struct you
need to use the `group!()` macro so that it can be encoded and decoded. An example
of this is the following

> 🚩 **IMPORTANT** 🚩 Everything previously mentioned for creating packets also applies to creating
> groups so keep that in mind when creating them.

```rust
use blaze_pk::group;

group! {
    struct MyGroup {
        TEST: u32,
        STR: String,
        BLOB: Vec<u8>
    }
}

```