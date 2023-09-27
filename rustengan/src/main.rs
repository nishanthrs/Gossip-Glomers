use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::{Deserializer};
use std::io::{StdoutLock, Write};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    src: String,
    dest: String,
    body: MessageBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageBody {
    msg_id: Option<usize>,
    in_reply_to: Option<usize>,
    #[serde(flatten)]
    payload: Payload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag="type")]
#[serde(rename_all="snake_case")]
enum Payload {
    Echo { echo: String },
    EchoOk { echo: String },
    Init { node_id: String, node_ids: Vec<String> },
    InitOk {},
    Generate {},
    GenerateOk { id: String },
}

struct EchoNode {
    // Node in distributed system that handles echo functionality
    id: usize,
}

impl EchoNode {
    fn gen_unique_id(&self, dest_node_id: &str) -> String {
        /*
        ID will be generated as a string consisting of:
        1. Unix timestamp in seconds
        2. Node ID it's generated on
        3. Msg ID (auto-increment counter)
        This format ensures that even if there are multiple nodes generating this ID every second,
        these IDs are guaranteed to be unique and sortable

        NOTE[1]: The timestamp is not needed for the purposes of the challenge.
        However, it does allow the IDs to be sortable, which is a nice bonus feature.
        NOTE[2]: Inherent flaw in this design is that it's not a fixed amount of bits (due to node and message ID).
        We should use an integer of fixed size x bits for message ID.
        Note that this means we'd have to reset the ID back to 0 every second,
        meaning the limit of the system is generating 2^x IDs per second.
        We can increase this by a factor of 1000x by storing the unix timestamp in milliseconds.
        */
        let curr_ts = SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards?").as_secs();
        return format!("{}_{}_{}", curr_ts.to_string(), dest_node_id, self.id.to_string());
    }

    pub fn step(
        &mut self,
        input: Message,
        output: &mut StdoutLock
    ) -> anyhow::Result<()> {
        match input.body.payload {
            Payload::Init { .. } => {
                let reply = Message {
                    src: input.dest,
                    dest: input.src,
                    body: MessageBody {
                        msg_id: Some(self.id),
                        in_reply_to: match input.body.msg_id {
                            None => None,
                            Some(input_msg_id) => Some(input_msg_id),
                        },
                        payload: Payload::InitOk {},
                    }
                };
                serde_json::to_writer(&mut *output, &reply).context("Failed to write reply data to output: stdout.");
                output.write_all(b"\n").context("Failed to write newline to output: stdout.")?;
                self.id += 1;  // NOTE: If there are multiple threads calling EchoNode, might have to put a lock here
            },
            Payload::Echo { echo } => {
                let reply = Message {
                    src: input.dest,
                    dest: input.src,
                    body: MessageBody {
                        msg_id: Some(self.id),
                        in_reply_to: match input.body.msg_id {
                            None => None,
                            Some(input_msg_id) => Some(input_msg_id),
                        },
                        payload: Payload::EchoOk { echo },
                    }
                };
                serde_json::to_writer(&mut *output, &reply).context("Failed to write reply data to output: stdout.");
                output.write_all(b"\n").context("Failed to write newline to output: stdout.")?;
                self.id += 1;  // NOTE: If there are multiple threads calling EchoNode, might have to put a lock here
            },
            Payload::EchoOk { .. } => {
                // Raise exception if receiving an EchoOk message
                bail!("Received unexpected EchoOk message!");
            },
            Payload::InitOk { .. } => {
                // Raise exception if receiving an InitOk message
                bail!("Received unexpected InitOk message!");
            },
            Payload::Generate { .. } => {
                let unique_id = self.gen_unique_id(&input.dest);
                let reply = Message {
                    src: input.dest,
                    dest: input.src,
                    body: MessageBody {
                        msg_id: Some(self.id),
                        in_reply_to: match input.body.msg_id {
                            None => None,
                            Some(input_msg_id) => Some(input_msg_id),
                        },
                        payload: Payload::GenerateOk { id: unique_id },
                    }
                };
                serde_json::to_writer(&mut *output, &reply).context("Failed to write reply data to output: stdout.");
                output.write_all(b"\n").context("Failed to write newline to output: stdout.")?;
                self.id += 1
            },
            Payload::GenerateOk { .. } => {
                // Raise exception if receiving an GenerateOk message
                bail!("Received unexpected GenerateOk message!");
            },
        };
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin().lock();
    let inputs = Deserializer::from_reader(stdin).into_iter::<Message>();
    let mut stdout = std::io::stdout().lock();
    // let mut output = Serializer::new(stdout);

    let mut echo_node = EchoNode {
        id: 0,
    };
    for input in inputs {
        let input = input.context("Maelstrom input could not be deserialized!")?;
        echo_node.step(input, &mut stdout).context("Node step function failed")?;
    }

    Ok(())
}
