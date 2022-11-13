# ⚙️BlazePK

![License](https://img.shields.io/github/license/jacobtread/BlazePk-rs?style=for-the-badge)
![Cargo Version](https://img.shields.io/crates/v/blaze-pk?style=for-the-badge)
![Cargo Downloads](https://img.shields.io/crates/d/blaze-pk?style=for-the-badge)

Rust library for working with the Blaze packet system this is the networking solution used by games such as
Mass Effect 3, Battlefield 3, another Other EA games. 

This is created for use with the Rust re-write of the [PocketRelay (https://github.com/jacobtread/PocketRelay)](https://github.com/jacobtread/PocketRelay) 
software.

## Crate Features
- default *Default features uses async*
- sync *Provides functions for syncronously writing and reading packets*
- async *Provides functions for asyncronously writing and reading packets through tokio*
- blaze-ssl *Provides functions for asyncronously writing and reading packets through Blaze-SSL streams*

## Working with structs

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

> VarInt = u8, u16, u32, u64, i8, i16, i32, i64, usize, isize

| Type                           | Details                                         |
|--------------------------------|-------------------------------------------------|
| VarInt                         | Encoded using variable length int encoding      |
| String                         | Text encoded will a null terminator             |
| Blob                           | Blob of bytes (wrapps a Vec of u8)              |
| Group                          | Group created with the group!() macro           |
| Vec<String,VarInt,Float,Group> | List of values                                  |
| TdfMap<String,VarInt, Any*>    | Map of keys to values                           |
| TdfOptional<Any*>              | Tdf value where value could be absent           |
| VarIntList                     | List of variable length integers                |
| (VarInt, VarInt)               | Pair of two VarInts                             |
| (VarInt, VarInt, VarInt)       | Tuple of three VarInts                          |
| f32                            | Only floating point type supported              |
| bool                           | Booleans are VarInts encoded with Zeros or Ones |


Fields are specified with {TAG} {NAME}: {TYPE} where {TAG} is the actual TDF tag
(This must be uppercase and must be in the correct order) and {NAME} is the name of
the Rust struct field

```rust
use blaze_pk::packet;

packet! {
    struct MyPacket {
        TEST test: u32,
        STR string: String,
        BLOB blob: Vec<u8>
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
        TEST test: u32,
        STR string: String,
        BLOB blob: Vec<u8>
    }
}

```

This struct can then be used in your packet struct


```rust
use blaze_pk::{packet, group};

group! {
    struct MyGroup {
        TEST test: u32,
        STR string: String,
        BLOB blob: Vec<u8>
    }
}

packet! {
    struct MyPacket {
        TEST test: u32,
        STR string: String,
        MY my_group: MyGroup
    }
}
```

## Creating packets

### Definining a component enum
In order to be able to send notify and request packets you will need to have an
enum for turning Component and Command names into IDs for this you can use the
`define_components!()` macro

The example usage of this macro is as follows:

```rust
use blaze_pk::define_components;

define_components! {
    Authentication (0x0) {
        First (0x1)
        Second (0x2)
        Third (0x3)
    }

    Other (0x1) {
        First (0x1)
        Second (0x2)
        Third (0x3)
    }
}
```

This will generate an enum named Components which contains mappings to all the 
```rust
#[derive(Debug, Eq, PartialEq)]
pub enum Components {
    Authentication(Authentication),
    Other(Other)
}

#[derive(Debug, Eq, PartialEq)]
pub enum Authentication {
    First,
    Second,
    Third,
    Unknown(u16),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Other {
    First,
    Second,
    Third,
    Unknown(u16),
}

```

### Creating a packet

Packets can be created with one of the following functions on the Packets struct. 

| Function | Arguments                                                                                                                  | Description                                           |
|----------|----------------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------|
| request  | counter (The request counter for ID's), component (A component line the one created earlier), content (The packet content) | Creates a request packet                              |
| notify   | component (A component line the one created earlier), content (The packet content)                                         | Creates a notification packet                         |
| response | packet (The packet to respond to), content (The packet content)                                                            | Creates a packet that is responding to another packet |
| error    | packet (The packet to respond to), error (The error), content (The packet content)                                         | Creates an error packet                               |

> Each of these functions have a variant suffixed with _empty which is a shortcut function 
> for specifiying one of these packets that has no content.

**Notify Example:**

This will create a Notify packet for the Authentication component with the First command

```rust

use blaze_pk::Packets;

// {Previous Code}

fn create_packet() {
    let my_packet = Packets::notify_empty(components::Authentication::First);
}
```
