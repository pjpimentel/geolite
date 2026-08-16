#[derive(prost::Message)]
pub struct blob_header_msg {
  #[prost(string, tag = "1")]
  pub r#type: String,
  #[prost(uint32, tag = "3")]
  pub datasize: u32,
}

#[derive(prost::Message)]
pub struct blob_msg {
  #[prost(bytes = "vec", optional, tag = "1")]
  pub raw: Option<Vec<u8>>,
  #[prost(int32, optional, tag = "2")]
  pub raw_size: Option<i32>,
  #[prost(bytes = "vec", optional, tag = "3")]
  pub zlib_data: Option<Vec<u8>>,
}

#[derive(prost::Message)]
pub struct header_block_msg {
  #[prost(message, optional, tag = "1")]
  pub bbox: Option<header_bbox_msg>,
  #[prost(string, repeated, tag = "4")]
  pub required_features: Vec<String>,
  #[prost(string, repeated, tag = "5")]
  pub optional_features: Vec<String>,
  #[prost(string, optional, tag = "16")]
  pub writingprogram: Option<String>,
  #[prost(string, optional, tag = "17")]
  pub source: Option<String>,
  #[prost(int64, optional, tag = "32")]
  pub osmosis_replication_timestamp: Option<i64>,
  #[prost(int64, optional, tag = "33")]
  pub osmosis_replication_sequence_number: Option<i64>,
  #[prost(string, optional, tag = "34")]
  pub osmosis_replication_base_url: Option<String>,
}

#[derive(prost::Message)]
pub struct header_bbox_msg {
  #[prost(sint64, tag = "1")]
  pub left: i64,
  #[prost(sint64, tag = "2")]
  pub right: i64,
  #[prost(sint64, tag = "3")]
  pub top: i64,
  #[prost(sint64, tag = "4")]
  pub bottom: i64,
}

#[derive(prost::Message)]
pub struct string_table_msg {
  #[prost(bytes = "vec", repeated, tag = "1")]
  pub s: Vec<Vec<u8>>,
}

#[derive(prost::Message)]
pub struct primitive_block_msg {
  #[prost(message, optional, tag = "1")]
  pub stringtable: Option<string_table_msg>,
  #[prost(message, repeated, tag = "2")]
  pub primitivegroup: Vec<primitive_group_msg>,
  #[prost(int32, optional, tag = "17")]
  pub granularity: Option<i32>,
  #[prost(int64, optional, tag = "19")]
  pub lat_offset: Option<i64>,
  #[prost(int64, optional, tag = "20")]
  pub lon_offset: Option<i64>,
  #[prost(int32, optional, tag = "18")]
  pub date_granularity: Option<i32>,
}

#[derive(prost::Message)]
pub struct primitive_group_msg {
  #[prost(message, repeated, tag = "1")]
  pub nodes: Vec<node_msg>,
  #[prost(message, optional, tag = "2")]
  pub dense: Option<dense_nodes_msg>,
  #[prost(message, repeated, tag = "3")]
  pub ways: Vec<way_msg>,
  #[prost(message, repeated, tag = "4")]
  pub relations: Vec<relation_msg>,
}

#[derive(prost::Message)]
pub struct info_msg {
  #[prost(int32, optional, tag = "1", default = "-1")]
  pub version: Option<i32>,
  #[prost(int64, optional, tag = "2")]
  pub timestamp: Option<i64>,
  #[prost(int64, optional, tag = "3")]
  pub changeset: Option<i64>,
  #[prost(int32, optional, tag = "4")]
  pub uid: Option<i32>,
  #[prost(uint32, optional, tag = "5")]
  pub user_sid: Option<u32>,
  #[prost(bool, optional, tag = "6")]
  pub visible: Option<bool>,
}

#[derive(prost::Message)]
pub struct dense_info_msg {
  #[prost(int32, repeated, tag = "1")]
  pub version: Vec<i32>,
  #[prost(sint64, repeated, tag = "2")]
  pub timestamp: Vec<i64>,
  #[prost(sint64, repeated, tag = "3")]
  pub changeset: Vec<i64>,
  #[prost(sint32, repeated, tag = "4")]
  pub uid: Vec<i32>,
  #[prost(sint32, repeated, tag = "5")]
  pub user_sid: Vec<i32>,
  #[prost(bool, repeated, tag = "6")]
  pub visible: Vec<bool>,
}

#[derive(prost::Message)]
pub struct node_msg {
  #[prost(sint64, tag = "1")]
  pub id: i64,
  #[prost(uint32, repeated, tag = "2")]
  pub keys: Vec<u32>,
  #[prost(uint32, repeated, tag = "3")]
  pub vals: Vec<u32>,
  #[prost(message, optional, tag = "4")]
  pub info: Option<info_msg>,
  #[prost(sint64, tag = "8")]
  pub lat: i64,
  #[prost(sint64, tag = "9")]
  pub lon: i64,
}

#[derive(prost::Message)]
pub struct dense_nodes_msg {
  #[prost(sint64, repeated, tag = "1")]
  pub id: Vec<i64>,
  #[prost(message, optional, tag = "5")]
  pub denseinfo: Option<dense_info_msg>,
  #[prost(sint64, repeated, tag = "8")]
  pub lat: Vec<i64>,
  #[prost(sint64, repeated, tag = "9")]
  pub lon: Vec<i64>,
  #[prost(int32, repeated, tag = "10")]
  pub keys_vals: Vec<i32>,
}

#[derive(prost::Message)]
pub struct way_msg {
  #[prost(int64, tag = "1")]
  pub id: i64,
  #[prost(uint32, repeated, tag = "2")]
  pub keys: Vec<u32>,
  #[prost(uint32, repeated, tag = "3")]
  pub vals: Vec<u32>,
  #[prost(message, optional, tag = "4")]
  pub info: Option<info_msg>,
  #[prost(sint64, repeated, tag = "8")]
  pub refs: Vec<i64>,
  #[prost(sint64, repeated, tag = "9")]
  pub lat: Vec<i64>,
  #[prost(sint64, repeated, tag = "10")]
  pub lon: Vec<i64>,
}

#[derive(prost::Message)]
pub struct relation_msg {
  #[prost(int64, tag = "1")]
  pub id: i64,
  #[prost(uint32, repeated, tag = "2")]
  pub keys: Vec<u32>,
  #[prost(uint32, repeated, tag = "3")]
  pub vals: Vec<u32>,
  #[prost(message, optional, tag = "4")]
  pub info: Option<info_msg>,
  #[prost(int32, repeated, tag = "8")]
  pub roles_sid: Vec<i32>,
  #[prost(sint64, repeated, tag = "9")]
  pub memids: Vec<i64>,
  #[prost(int32, repeated, tag = "10")]
  pub types: Vec<i32>,
}
