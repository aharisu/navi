use navi::value as NaviValue;

fn main() {
    /*
    let one = Value::One;
    let two = Value::Two(false);
    let three = Value::Three(777);
    let four = Value::Four(String::from("rarar"));
    let ary = [Value::One, Value::Two(true), Value::Three(1024), Value::Four(String::from("hogehoge"))];

    println!("Hello, world!! {}", ary.len());

    println!("{}", mem::size_of_val(&one));
    println!("{}", mem::size_of_val(&two));
    println!("{}", mem::size_of_val(&three));
    println!("{}", mem::size_of_val(&four));
    println!("{}", mem::size_of::<Value>());
    println!("{}", mem::size_of::<bool>());
    println!("{}", mem::size_of::<i32>());
    println!("{}", mem::size_of::<String>());
    println!("{}", mem::size_of::<TypeId>());
    println!("{}", TypeId::A as u32);
    println!("{}", TypeId::B as u32);
    println!("{}", TypeId::E as u32);
    println!("{}", std::mem::size_of::<usize>());
    */

    //println!("word_size:{}, ptr_size:{}", std::mem::size_of::<usize>(), std::mem::size_of::<*const u8>());
    //println!("{}, {}", std::mem::align_of::<V1>(), std::mem::align_of::<V2>());

    //let word_size = std::mem::size_of::<usize>();
    //let layout = std::alloc::Layout::from_size_align(1024, word_size).unwrap();
    //unsafe {
    //    let ptr = std::alloc::alloc(layout);
    //    let ptr_v = ptr as usize;
    //    println!("{}", ptr_v);
    //    let mut v1 = &mut *(ptr as *mut V1);
    //    //let v1: &V1 = (ptr as *mut V1).as_ref().unwrap();
    //    v1.b = true;
    //    println!("{}", v1.b);
    //    println!("{}, {}", ptr as u64, v1 as *const V1 as u64);

    //    let next = ptr.offset(1);
    //    println!("{}, next:{}", ptr as u32, next as u32);

    //    std::alloc::dealloc(ptr as *mut u8, layout);
    //}

    //let v1 = V1::alloc(&mut heap);
    //v1.b = false;
    //println!("{}", v1.b);
    //v1.b = true;
    //println!("{}", v1.b);

    //let v1_2 = V1::alloc(&mut heap);
    //v1_2.b = false;
    //println!("{}, {}", v1.b, v1_2.b);
    //v1_2.b = true;
    //println!("{}, {}", v1.b, v1_2.b);

    //let v2 = V2::alloc(&mut heap);
    //v2.b = true;
    //v2.num = 100;
    //println!("{}, {}", v2.b, v2.num);

    //println!("{}", (navi::number::NUMBER_HEADER.size_of.unwrap())(&navi::object::Number{num: 101}));

    let mut heap = navi::object::mm::Heap::new();
    /*
    let num = NaviValue::alloc::<navi::t::number::Number>(&mut heap);
    println!("{}", num.num);
    let num2 = NaviValue::alloc::<navi::t::number::Number>(&mut heap);
    println!("{}", num2.num);
    */

    /*
    std::mem::size_of_val_raw(std::alloc::alloc());

    unsafe {
        std::alloc::alloc(layout: Layout)(layout);
    }
    println!("{}", .unwrap().size());
    println!("{}", std::alloc::Layout::from_size_align(9, word_size).unwrap().size());
    println!("{}", std::alloc::Layout::from_size_align(17, word_size).unwrap().size());
    println!("{}", std::alloc::Layout::from_size_align(2, word_size).unwrap().size());
    */

    use std::ptr;

    // create a slice pointer when starting out with a pointer to the first element
    let mut x  = [97u8, 98u8, 99u8];
    let raw_pointer = x.as_mut_ptr();
    let slice = unsafe{&*(ptr::slice_from_raw_parts_mut(raw_pointer, 3))};
    let str = std::mem::ManuallyDrop::new(unsafe { std::string::String::from_raw_parts(raw_pointer, 3, 3) });
    let str_ref = &str;

    let r = '0' ..= '9';

    for i in r {
        println!("{}", i);
    }

    println!("{:?}", x);
    println!("{:?}", slice);
    println!("{:?}", str);
    println!("{:?}", str_ref);

    x[0] = 100;

    println!("{:?}", x);
    println!("{:?}", slice);
    println!("{:?}", str);
    println!("{:?}", str_ref);

    //println!("{:?}", vec);

}
