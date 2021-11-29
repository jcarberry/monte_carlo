use std::{any::{Any, TypeId}, ops::Deref, ptr::null, rc::Rc};
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use rand::prelude::{Distribution, ThreadRng, thread_rng};
use statrs::distribution::{Exp, Gamma, Weibull};

const STATUS_ALIVE: usize = 0;
const STATUS_DEAD: usize = 1;
const STATUS_DYNAMIC: usize = 2;

const DIST_EXP: usize = 0;
const DIST_WEIBULL: usize = 1;
const DIST_GAMMA: usize = 2;
const DIST_NONE: usize = 3;

const FT_BASIC: usize = 0;
const FT_STATIC: usize = 1;
const FT_SEQUENTIAL: usize = 2;

const EVENT_FAILURE: usize = 0;
const EVENT_REPAIR: usize = 1;

enum DistributionType {
    Exp(Exp),
    Gamma(Gamma),
    Weibull(Weibull),
    None,
}

trait FTElement {
    fn get_failed(&self, ft: &FT) -> bool;
    fn get_id(&self) -> usize;
    fn get_type(&self) -> usize;
    fn set_status(&mut self, status: usize);
    fn as_any(&self) -> &dyn Any;
}

pub struct FT {
    root: usize,
    elements: Vec<Box<dyn FTElement>>,
}

impl FT {
    fn new() -> FT {
        FT{root: 0, elements: Vec::new()}
    }
    fn add_element(&mut self, element: Box<dyn FTElement>) {
        self.elements.insert(element.get_id(), element)
    }
    fn get_failed(&self, element: usize) -> bool {
        self.elements.get(element)
            .unwrap()
            .get_failed(self)
    }
    fn get_basic_events(&self)  -> Vec<usize> {
        let mut basic_events = Vec::new();
        for (i, element) in self.elements.iter().enumerate() {
            if element.get_type() == FT_BASIC {
                basic_events.push(i);
            }
        }
        basic_events
    }
    fn sample_failure(&self, element: usize, r: &mut ThreadRng) -> Result<f64, &'static str> {
        let element = self.elements.get(element).unwrap();
        if element.get_type() == FT_BASIC {
            let element = element.as_any().downcast_ref::<BasicEvent>().expect("Not a basic event");
            Ok(element.sample_failure(r))
        } else {
            Err("Not a basic event")
        }
    }
    fn sample_repair(&self, element: usize, r: &mut ThreadRng) -> Result<f64, &'static str> {
        let element = self.elements.get(element).unwrap();
        if element.get_type() == FT_BASIC {
            let element = element.as_any().downcast_ref::<BasicEvent>().expect("Not a basic event");
            Ok(element.sample_repair(r))
        } else {
            Err("Not a basic event")
        }
    }
    fn process_event_time(&mut self, event_time: EventTime) {
        match event_time.event_type {
            EVENT_FAILURE => {
                self.elements.get_mut(event_time.element).unwrap().set_status(STATUS_DEAD);
            }
            EVENT_REPAIR => {
                self.elements.get_mut(event_time.element).unwrap().set_status(STATUS_ALIVE);
                //update parent elements if they are sequential
            }
            _ => {
                panic!("Unknown event type")
            }
        };
    }
    fn reset_basic_events(&mut self) {
        let basic_events = self.get_basic_events();
        for i in basic_events {
            self.elements.get_mut(i).unwrap().set_status(STATUS_ALIVE);
        };
    }
}

pub struct BasicEvent {
    id: usize,
    status: usize,
    failure_distribution: DistributionType,
    repair_distribution: DistributionType,
}

impl FTElement for BasicEvent {
    fn get_failed(&self, ft: &FT) -> bool {
        match self.status {
            STATUS_ALIVE => false,
            STATUS_DEAD => true,
            _ => panic!("Incorrect event status")
        }
    }
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_type(&self) -> usize {
        FT_BASIC
    }
    fn set_status(&mut self, status: usize) {
        self.status = status;
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl BasicEvent {
    fn new(id: usize, failure_distribution: usize, failure_scale: f64, failure_shape: f64, 
            repair_distribution: usize, repair_scale: f64, repair_shape: f64) -> BasicEvent{
        let failure_distribution: DistributionType = match failure_distribution {
            DIST_EXP => {
                if failure_shape != 0.0 {
                    println!("Exponential distribution used - shape parameter ignored. Use shape = 0.0 to suppress this message.");
                }
                DistributionType::Exp(Exp::new(failure_scale).unwrap())
            }
            DIST_GAMMA => DistributionType::Gamma(Gamma::new(failure_shape, failure_scale).unwrap()),
            DIST_WEIBULL => DistributionType::Weibull(Weibull::new(failure_shape, failure_scale).unwrap()),
            _ => panic!("Incorrect failure distribution type")
        };
        let repair_distribution: DistributionType = match repair_distribution {
            DIST_EXP => {
                if repair_shape != 0.0 {
                    println!("Exponential distribution used - shape parameter ignored. Use shape = 0.0 to suppress this message.");
                }
                DistributionType::Exp(Exp::new(repair_scale).unwrap())
            }
            DIST_GAMMA => DistributionType::Gamma(Gamma::new(repair_shape, repair_scale).unwrap()),
            DIST_WEIBULL => DistributionType::Weibull(Weibull::new(repair_shape, repair_scale).unwrap()),
            DIST_NONE => {
                if repair_shape != 0.0 || repair_scale != 0.0 {
                    println!("None distribution used - shape and rate parameters ignored. Use shape = rate = 0.0 to suppress this message.");
                }
                DistributionType::None
            },
            _ => panic!("Incorrect repair distribution type")
        };
        BasicEvent{id, status: STATUS_ALIVE, failure_distribution, repair_distribution}
    }
    fn sample_failure(&self, r: &mut ThreadRng) -> f64 {
        match self.failure_distribution {
            DistributionType::Exp(d) => d.sample(r),
            DistributionType::Weibull(d) => d.sample(r),
            DistributionType::Gamma(d) => d.sample(r),
            _ => panic!("Cannot sample a none distribution"),
        }
    }
    fn sample_repair(&self, r: &mut ThreadRng) -> f64 {
        match self.repair_distribution {
            DistributionType::Exp(d) => d.sample(r),
            DistributionType::Weibull(d) => d.sample(r),
            DistributionType::Gamma(d) => d.sample(r),
            DistributionType::None => 0.0,
        }
    }
}

pub struct Children {
    children: Vec<usize>,
}

impl Children {
    fn new() -> Children {
        Children{children: Vec::new()}
    }
    fn get(&self) -> &Vec<usize> {
        &self.children
    }
    fn add(&mut self, child: &dyn FTElement) {
        self.children.push(child.get_id());
    }
}

pub struct GateAnd {
    id: usize,
    children: Children,
}

impl GateAnd {
    fn new(id: usize) -> GateAnd {
        GateAnd{id, children: Children::new()}
    }
}

impl FTElement for GateAnd {
    fn get_failed(&self, ft: &FT) -> bool {
        for child in self.children.get() {
            if !ft.get_failed(*child) {
                return false
            }
        }
        true
    }
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_type(&self) -> usize {
        FT_STATIC
    }
    fn set_status(&mut self, status: usize) {
        panic!("Cannot manually set AND gate status")
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct GateOr {
    id: usize,
    children: Children,
}

impl GateOr {
    fn new(id: usize) -> GateOr {
        GateOr{id, children: Children::new()}
    }
}

impl FTElement for GateOr {
    fn get_failed(&self, ft: &FT) -> bool {
        for child in self.children.get() {
            if ft.get_failed(*child) == true {
                return true
            }
        }
        false
    }
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_type(&self) -> usize {
        FT_STATIC
    }
    fn set_status(&mut self, status: usize) {
        panic!("Cannot manually set OR gate status")
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct GateVote {
    id: usize,
    children: Children,
}

impl GateVote {
    fn new(id: usize) -> GateVote {
        GateVote{id, children: Children::new()}
    }
}

impl FTElement for GateVote {
    fn get_failed(&self, ft: &FT) -> bool {
        let threshold: usize = self.children.get().len() / 2;
        let mut failed: usize = 0;
        for child in self.children.get() {
            if ft.get_failed(*child) == true {
                failed += 1;
            }
        }
        failed > threshold
    }
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_type(&self) -> usize {
        FT_STATIC
    }
    fn set_status(&mut self, status: usize) {
        panic!("Cannot manually set VOTE gate status")
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct IDGenerator {
    counter: usize,
}

impl IDGenerator {
    fn new() -> IDGenerator {
        IDGenerator{counter: 0}
    }
    fn get_next(&mut self) -> usize {
        self.counter += 1;
        self.counter - 1
    }
}

pub struct EventTime {
    time: f64,
    element: usize,
    event_type: usize,
}

fn main() {
    let mut id_gen = IDGenerator::new();
    let mut ft = FT::new();

    // let c1: BasicEvent = BasicEvent::new(id_gen.get_next(),
    //  DIST_WEIBULL, 100.0, 1.5, 
    //  DIST_NONE, 0.0, 0.0);
    // let c2: BasicEvent = BasicEvent::new(id_gen.get_next(),
    // DIST_WEIBULL, 100.0, 1.5, 
    //  DIST_NONE, 0.0, 0.0);
    // let c3: BasicEvent = BasicEvent::new(id_gen.get_next(),
    // DIST_WEIBULL, 100.0, 1.5, 
    //  DIST_NONE, 0.0, 0.0);

    // let mut g2: GateVote = GateVote::new(id_gen.get_next());
    // g2.children.add(&c1);
    // g2.children.add(&c2);
    // g2.children.add(&c3);

    // ft.add_element(Box::new(c1));
    // ft.add_element(Box::new(c2));
    // ft.add_element(Box::new(c3));

    // let root_id = g2.get_id();
    // ft.add_element(Box::new(g2));

    // let c1: BasicEvent = BasicEvent::new(id_gen.get_next(),
    //  DIST_EXP, 1.0/100.0, 0.0, 
    //  DIST_EXP, 1.0/100.0, 0.0);
    // let c2: BasicEvent = BasicEvent::new(id_gen.get_next(),
    // DIST_EXP, 1.0/100.0, 0.0, 
    // DIST_EXP, 1.0/100.0, 0.0);

    // let mut g1: GateAnd = GateAnd::new(id_gen.get_next());
    // g1.children.add(&c1);
    // g1.children.add(&c2);

    // ft.add_element(Box::new(c1));
    // ft.add_element(Box::new(c2));

    // let root_id = g1.get_id();
    // ft.add_element(Box::new(g1));

    let c1: BasicEvent = BasicEvent::new(id_gen.get_next(),
     DIST_EXP, 1.0/100.0, 0.0, 
     DIST_EXP, 1.0/100.0, 0.0);
    let c2: BasicEvent = BasicEvent::new(id_gen.get_next(),
    DIST_EXP, 1.0/100.0, 0.0, 
    DIST_EXP, 1.0/100.0, 0.0);
    let c3: BasicEvent = BasicEvent::new(id_gen.get_next(),
     DIST_EXP, 1.0/100.0, 0.0, 
     DIST_EXP, 1.0/100.0, 0.0);
    let c4: BasicEvent = BasicEvent::new(id_gen.get_next(),
    DIST_EXP, 1.0/100.0, 0.0, 
    DIST_EXP, 1.0/100.0, 0.0);
    let c5: BasicEvent = BasicEvent::new(id_gen.get_next(),
     DIST_EXP, 1.0/100.0, 0.0, 
     DIST_EXP, 1.0/100.0, 0.0);
    let c6: BasicEvent = BasicEvent::new(id_gen.get_next(),
    DIST_EXP, 1.0/100.0, 0.0, 
    DIST_EXP, 1.0/100.0, 0.0);

    let mut g1: GateAnd = GateAnd::new(id_gen.get_next());
    g1.children.add(&c1);
    g1.children.add(&c2);
    let mut g2: GateAnd = GateAnd::new(id_gen.get_next());
    g2.children.add(&c3);
    g2.children.add(&c4);
    let mut g3: GateAnd = GateAnd::new(id_gen.get_next());
    g3.children.add(&c5);
    g3.children.add(&c6);

    ft.add_element(Box::new(c1));
    ft.add_element(Box::new(c2));
    ft.add_element(Box::new(c3));
    ft.add_element(Box::new(c4));
    ft.add_element(Box::new(c5));
    ft.add_element(Box::new(c6));

    let mut g4: GateVote = GateVote::new(id_gen.get_next());
    g4.children.add(&g1);
    g4.children.add(&g2);
    g4.children.add(&g3);

    ft.add_element(Box::new(g1));
    ft.add_element(Box::new(g2));
    ft.add_element(Box::new(g3));

    let root_id = g4.get_id();
    ft.add_element(Box::new(g4));

    let basic_events = ft.get_basic_events();
    let mut rng = rand::thread_rng();

    let mut out_string = String::new();

    for _ in 0..10000 {
        let mut event_times: Vec<EventTime> = Vec::new();

        for element in &basic_events {
            let element = *element;
            let failure_time = ft.sample_failure(element, &mut rng).unwrap();
            let event_time = EventTime{time: failure_time, element, event_type: EVENT_FAILURE};
            let mut index = 0;
            while index < event_times.len() && failure_time > event_times.get(index).unwrap().time {
                index += 1;
            };
            event_times.insert(index, event_time);

        };

        loop {
            let next_event_time = event_times.remove(0);
            let time = next_event_time.time;
            let element = next_event_time.element;
            let event_type = next_event_time.event_type;
            //println!("Component {} failed at t = {}", element, time);
            ft.process_event_time(next_event_time);
            
            if event_type == EVENT_FAILURE {
                if ft.get_failed(root_id)  {
                    //println!("System failed at t = {}", time);
                    print!("{} ", time);
                    out_string.push_str(&time.to_string());
                    out_string.push(' ');
                    break;
                } else {
                    let repair_interval = ft.sample_repair(element, &mut rng).unwrap();
                    let repair_time = time + repair_interval;
                    if repair_interval > 0.0 {
                        let event_time = EventTime{time: repair_time, element, event_type: EVENT_REPAIR};
                        let mut index = 0;
                        while index < event_times.len() && repair_time > event_times.get(index).unwrap().time {
                            index += 1;
                        };
                        event_times.insert(index, event_time);
                    }
                }
            } else if event_type == EVENT_REPAIR {
                let failure_interval = ft.sample_failure(element, &mut rng).unwrap();
                let failure_time = time + failure_interval;
                let event_time = EventTime{time: failure_time, element, event_type: EVENT_FAILURE};
                let mut index = 0;
                while index < event_times.len() && failure_time > event_times.get(index).unwrap().time {
                    index += 1;
                };
                event_times.insert(index, event_time);
            }
        }

        ft.reset_basic_events()
    }
    let out_string = out_string.trim_end();
    let path = Path::new("output.txt");
    let mut file = File::create(&path).expect("File creation error");
    file.write(out_string.as_bytes()).expect("Write error");
    println!("done");
  
    // hot spare with switch
    // if switch is failed, gate cannot access spares
    // list of spares
    // boolean if it has failed (must encode some data about failure, cant be inferred like logical gates)
    // when switch/primary/spares gets repaired, must update hot spare

    /*
    spare gate {
        children = list of children components (primary+spare) ordered by priority
        available children = bitmap mapping children availability
        switch = switch component
        current = current component //when something fails, check if this has failed
        hasfailed() {}
        update(component, type) {
            if type == failure {
                if component == switch {
                    remove all children from available
                } else if component == current {

                    next = first TRUE in available
                    if next is null (no parts left) {
                        current = null
                        failed = true
                    } else {
                        current = next
                        //tell everyone else that this spare part is in use, cannot be taken
                    } else //component is in the queue of spares {
                        pop component from available
                    }
                }
            } else if type == repair {
                if component == switch {
                    add all children back IF THEY ARE ALIVE ONLY
                } else //component should never be current, since current is alive {
                    add component back to available IN CORRECT PRIORITY ORDER (how?) (maybe bitmap easier?)
                }
            }
        }
    }*/
}
