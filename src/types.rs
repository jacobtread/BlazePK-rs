trait BlazeSerialize {

}

trait BlazeDeserialize {

}

#[derive(BlazeSerialize)]
struct ExamplePacket {
    #[label = "TEST"]
    test: String,
    #[label = "NEST"]
    nested: Nested
}

#[derive(BlazeSerialize)]
struct Nested {

}