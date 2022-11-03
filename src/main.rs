extern crate timer;
extern crate chrono;

/*
How it works locally
Assume we have a list of ips:
Create a NODE for each of them
Each node should
- store raft info? what info?
- be able to start or stop
- know about the rest of the nodes in the cluster, (address and count)
- be able to send data to other nodes, or broadcast to all nodes
- have a run function that acts depends on state (calls follower state)
- have a queue of requests it receives and responds to
*/

struct Node<S> {
    // currentTerm
    // votedFor
    // log[]
    // timer: timer::Timer
    state: S
}

// try testing if adding raftstate also works here
impl Node<Follower> {
    fn new() -> Node<Follower> {
        Node {
            state: Follower {
                timer: timer::Timer::new()
            }
        }
    }
}
impl RaftState for Node<Follower>{
    fn run(&self) {

        // while timer has not timed out, do stuff
        // when timer has timed out, transition to candidate state

        // for some reason this doesn't print unless stays in function
        // afterwards
        // is this because the whole context is deleted after all lines have
        // been read through??
        let _guard = self.state.timer.schedule_with_delay(chrono::Duration::milliseconds(100), || {
            println!("timed out");
        });
        _guard.ignore();
        println!("end follower run");
        // loop {
            
        // }
    }
}

// specify functions that raft state must implement
// do this for readability
// but i also want to do Node::run(), which decides which state run to use
// T_T
pub trait RaftState {
    fn run(&self);
}

// idk if you can do this actually...
// impl<S: RaftState> Node<S> {
//     fn run(&self) {
//         // start gRPC server

//         loop {
//             self.state.run()
//         }
//     }
//     // run() - match State with follower, candidate, or leader
//     // and execute state specific run function
//     // state specific run function should handle state transition?
//     // we want to do state transitions when timeout or received a message
//     // e.g. if Leader receives a response with higher term number, it becomes a follower
// }

struct Follower {
    timer: timer::Timer
}

struct Candidate {
    timer: timer::Timer
}

impl Node<Candidate> {
    fn new() -> Node<Candidate> {
        Node {
            state: Candidate {
                timer: timer::Timer::new()
            }
        }
    }
}
impl RaftState for Node<Candidate>{
    fn run(&self) {
        println!("running candidate");
        // loop {
        // }
    }
}

impl From<Node<Follower>> for Node<Candidate> {
    fn from(state: Node<Follower>) -> Node<Candidate> {
        Node {
            state: Candidate {
                timer: timer::Timer::new()
            }
        }
    }
}
  
fn main() {

    // test the transitions to different states
    // create a node (starts as follower)
    let node = Node::<Follower>::new();

    node.run();

    println!("escaped run");

    // transition follower to candidate
    let node = Node::<Candidate>::from(node);

    node.run();

    // transition candidate to leader

    // transition back to follower (bc becomes disconnected for example)

    // time test code
    // let t = timer::Timer::new();
    // let _guard = t.schedule_with_delay(chrono::Duration::milliseconds(100), || {
    //     println!("timed out");
    // });

    loop {

    }

}
