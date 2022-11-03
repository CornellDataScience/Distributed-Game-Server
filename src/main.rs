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


// on state machines
// Easier implementation:
// have an enum and pattern match
// node that holds the enum has all the functionality and vars needed for all
// states
// match on the enum
// Harder implementation:
// State structs - hold specific data
// Node struct - hold common data
// still need an enum wrapper???
//
// enum = 
// _(struct)
// loop {
//     let e = match e {
//         F x => x.run (evaluates to e using from(self))
//         C x => .run
//         L x => .run
//     }
// }
// but i cant get run to evaluate to the right thing for 

pub enum State {
    F(Node<Follower>),
    C(Node<Candidate>),
    Done
}

pub struct Node<S> {
    state: S
}

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
    fn run(self) -> State {

        // while timer has not timed out, do stuff
        // when timer has timed out, transition to candidate state

        // TODO: if this guard is not maintained, the timer disappears too...

        // let _guard = self.state.timer.schedule_with_delay(chrono::Duration::milliseconds(100), || {
        //     println!("timed out");
        //     let node = Node::<Candidate>::from(self);
        //     return State::C(node)
        // });
        // _guard.ignore();

        // loop {
        //     println!("running");
        // }
        
        // transition
        println!("follower");
        let node = Node::<Candidate>::from(self);
        return State::C(node)
    }
}

pub trait RaftState {
    fn run(self) -> State;
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

pub struct Follower {
    timer: timer::Timer
}

pub struct Candidate {
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
    fn run(self) -> State {
        println!("running candidate");

        State::Done
        // State::C(self)
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

    // time test code
    // let t = timer::Timer::new();
    // let _guard = t.schedule_with_delay(chrono::Duration::milliseconds(100), || {
    //     println!("timed out");
    // });
    // loop {

    // }

    // test the transitions to different states
    // create a node (starts as follower)
    let node = Node::<Follower>::new();

    node.run();

    println!("escaped run");

    // transition follower to candidate
    // let node = Node::<Candidate>::from(node);

    // node.run();

    // transition candidate to leader

    // transition back to follower (bc becomes disconnected for example)

    let mut s = State::F(Node::<Follower>::new());
    loop {
        s = match s {
            State::F(x) => x.run(),
            State::C(x) => x.run(),
            State::Done => { println!("Done"); break }
        };
    }

}
