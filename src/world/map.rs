use std::fmt::Debug;

pub struct PatriciaTree<T: Debug> {
    children: Vec<Node<T>>,
}

pub struct Node<T: Debug> {
    prefix: String,
    value: Option<T>,
    children: Vec<Node<T>>,
}

impl <T: Debug> Node<T> {
    pub fn value_as_ref(&self) -> Option<&T> {
        self.value.as_ref()
    }

}

impl <T: Debug> PatriciaTree<T> {
    pub fn new() -> Self {
        PatriciaTree {
            children: Vec::new(),
        }
    }

    pub fn add<K: AsRef<str>>(&mut self, key: K, v: T) {
        let key = key.as_ref();
        match self.children.binary_search_by(|n| Self::cmp_first_char(n, key)) {
            Ok(index) => {
                let (target_node, node_used, key_used) = Self::longest_match_mut(&mut self.children[index], key);

                //ルートノードから一切対応するノードが見つからなかった場合
                if key_used == 0 {
                    // continue

                } else if node_used == target_node.prefix.len() && key_used == key.len() {
                    //ノードにもキーにも余りの文字がない場合は完全一致の既存ノードが見つかった
                    target_node.value = Some(v);

                    return;

                } else if key_used == key.len() {
                    //キーに一致するノードを見つけたが対応するノードの文字が余っている
                    Self::split_node(target_node, node_used);
                    target_node.value = Some(v);
                    return;

                } else {
                    //キーの文字に対応するノードがまだなかった
                    if node_used != 0 {
                        Self::split_node(target_node, node_used);
                    }
                    let new_node = Node {
                        prefix: String::from(&key[key_used..]),
                        value: Some(v),
                        children: Vec::new(),
                    };
                    Self::add_node(target_node, new_node);
                    return;
                }
            }
            Err(index) => {
                self.children.insert(index, Node {
                    prefix: String::from(key),
                    value: Some(v),
                    children: Vec::new(),
                });
            }
        }

    }

    pub fn get<K: AsRef<str>>(&self, key: K) -> Option<&T> {
        let key = key.as_ref();
        match self.children.binary_search_by(|n| Self::cmp_first_char(n, key)) {
            Ok(index) => {
                match Self::complete_match(&self.children[index], key) {
                    Some(node) => match &node.value {
                        Some(v) => Some(v),
                        None => None,
                    }
                    None => None,
                }
            }
            _ => None
        }
    }

    #[allow(dead_code)]
    pub fn delete<K: AsRef<str>>(&mut self, key: K) {
        fn delete_node_rec<'a, T: Debug>(node: &'a mut Node<T>, key: &str) -> bool {
            let mut key_iter = key.chars();
            let mut node_iter = node.prefix.chars();
            let mut indexer = 0..;

            enum Kind {
                None,
                SelfDelete,
                ChildDelete(usize),
            }
            let kind = loop {
                match (key_iter.next(), node_iter.next(), indexer.next()) {
                    (Some(key_char), Some(node_char), Some(_)) if key_char == node_char => {
                        //keyとnodeの部分文字列がまだ一致しているのでループを継続
                    }
                    (Some(_), None, Some(index)) => {
                        //nodeが持つ文字列と完全に一致して、key側に文字が残った状態。

                        //次のノードを探す
                        let key_remainder = String::from(&key[index..]);
                        match node.children.binary_search_by(|child| PatriciaTree::cmp_first_char(child, &key_remainder)) {
                            Ok(next_node_index) => {
                                //プレフィックスに対応する次のノードが見つかったので再帰的に検索
                                if delete_node_rec(&mut node.children[next_node_index], &key_remainder) {
                                    break Kind::ChildDelete(next_node_index);
                                } else {
                                    break Kind::None;
                                }
                            }
                            Err(_) => {
                                //対応する次のノードが見つからなかったので検索失敗
                                break Kind::None;
                            }
                        }
                    }
                    (None, None, _) => {
                        //キーにマッチするノードを見つけたので削除処理を実行

                        if node.children.len() == 0 {
                            //葉ノードならノード自体を削除しても良いのでマークをつけておく
                            node.prefix.clear();

                        } else if node.children.len() == 1 {
                            //子要素を一つしかもっていないノードなら、削除対象ノードとその子ノードをマージする
                            let child = &mut std::mem::take(&mut node.children)[0];

                            //子が持っていた内容をすべて自ノードに統合する
                            node.prefix += &*child.prefix;
                            node.value = std::mem::take(&mut child.value);
                            node.children = std::mem::take(&mut child.children);
                        } else {
                            //複数の子ノードがいる場合はノード自体を削除することができないので、valueの値だけクリアする
                            node.value = None;
                        }

                        break Kind::SelfDelete;
                    }
                    _ => {
                        break Kind::None;
                    }
                }
            };

            match kind {
                Kind::ChildDelete(index) => {
                    //子ノードが削除されて戻ってきたので諸々のチェック
                    if node.children[index].prefix.is_empty() {
                        //葉ノードが削除された場合はChildrenの中から削除
                        node.children.remove(index);

                    }

                    if node.children.len() == 1
                        && (node.value.is_none() || node.children[0].value.is_none()) {
                        //削除されたノードしか子ノードを持っていない
                        //かつ、親子どちらかのノードが対応する値を持っていなければ
                        //マージする

                        let child = &mut std::mem::take(&mut node.children)[0];

                        //子が持っていた内容をすべて自ノードに統合する
                        node.prefix += &*child.prefix;
                        node.value = std::mem::take(&mut node.value).or(std::mem::take(&mut child.value));
                        node.children = std::mem::take(&mut child.children);
                    }

                    false
                }
                Kind::SelfDelete => true,
                Kind::None => false,
            }
        }

        let key = key.as_ref();
        match self.children.binary_search_by(|n| Self::cmp_first_char(n, key)) {
            Ok(index) => {
                let is_root_deleted = delete_node_rec(&mut self.children[index], key);
                if is_root_deleted {
                    self.children.remove(index);
                }
            }
            _ => { }
        }
    }

    fn longest_match_mut<'a>(node: &'a mut Node<T>, key: &str) -> (&'a mut Node<T>, usize, usize) {
        let mut key_iter = key.chars();
        let mut node_iter = node.prefix.chars();
        let mut indexer = 0..;

        loop {
            match (key_iter.next(), node_iter.next(), indexer.next()) {
                (Some(a), Some(b), Some(_)) if a == b => {
                    //keyとnodeの部分文字列がまだ一致しているのでループを継続
                }
                (Some(_), None, Some(index)) => {
                    //nodeが持つ文字列と完全に一致して、key側に文字が残った状態。

                    //次のノードを探す
                    let key_remainder = String::from(&key[index..]);
                    match node.children.binary_search_by(|child| Self::cmp_first_char(child, &key_remainder)) {
                        Ok(next_node_index) => {
                            //プレフィックスに対応する次のノードが見つかったので再帰的に検索
                            let (target_node, node_remainder, key_remainder) = Self::longest_match_mut(&mut node.children[next_node_index], &key_remainder);
                            break (target_node, node_remainder, key_remainder + index);
                        }
                        Err(_) => {
                            //次のノードはまだ存在しないので現在ノードを返す
                            break (node, index, index);
                        }
                    }
                }
                (_, _, Some(index)) => {
                    break (node, index, index);
                }
                _ => {
                    unreachable!()
                }
            }
        }
    }

    fn complete_match<'a>(node: &'a Node<T>, key: &str) -> Option<&'a Node<T>> {
        let mut key_iter = key.chars();
        let mut node_iter = node.prefix.chars();
        let mut indexer = 0..;

        loop {
            match (key_iter.next(), node_iter.next(), indexer.next()) {
                (Some(a), Some(b), Some(_)) if a == b => {
                    //keyとnodeの部分文字列がまだ一致しているのでループを継続
                }
                (Some(_), None, Some(index)) => {
                    //nodeが持つ文字列と完全に一致して、key側に文字が残った状態。

                    //次のノードを探す
                    let key_remainder = String::from(&key[index..]);
                    match node.children.binary_search_by(|child| Self::cmp_first_char(child, &key_remainder)) {
                        Ok(next_node_index) => {
                            //プレフィックスに対応する次のノードが見つかったので再帰的に検索
                            break Self::complete_match(&node.children[next_node_index], &key_remainder);
                        }
                        Err(_) => {
                            //対応する次のノードが見つからなかったので検索失敗
                            break None;
                        }
                    }
                }
                (None, None, _) => {
                    break Some(&node);
                }
                _ => {
                    break None;
                }
            }
        }
    }

    fn cmp_first_char(node: &Node<T>, key: &str) -> std::cmp::Ordering {
        match (node.prefix.chars().next(), key.chars().next()) {
            (Some(a), Some(b)) => a.cmp(&b),
            (Some(_), None) => std::cmp::Ordering::Greater,
            (None, Some(_)) => std::cmp::Ordering::Less,
            (None, None) => std::cmp::Ordering::Equal,
        }
    }

    fn split_node(node: &mut Node<T>, split_index: usize) {
        //余った文字に対応するノードを作成して、既存の情報をすべて移行する

        //所有権の問題で単純に置き換えができないのでsd::mem::replaceを使用する
        let old_children = std::mem::replace(&mut node.children, Vec::new());

        let new_node = Node {
                prefix: String::from(&node.prefix[split_index..]),
                value: std::mem::take(&mut node.value),
                children: old_children
            };

        node.prefix = String::from(&node.prefix[..split_index]);
        node.value = None;
        node.children.push(new_node);
    }

    fn add_node(target_node: &mut Node<T>, new_node: Node<T>) {
        match target_node.children.binary_search_by(|child| Self::cmp_first_char(child, &new_node.prefix)) {
            Err(index) => target_node.children.insert(index, new_node),
            _ => unreachable!()
        }
    }

    pub fn to_vec_preorder<'a>(&'a self) -> Vec::<&'a Node<T>> {
        fn rec<'a, T: Debug>(node: &'a Node<T>, acc: &mut Vec::<&'a Node<T>>) {
            acc.push(node);

            for c in &node.children {
                rec(c, acc);
            }
        }

        let mut v = Vec::new();
        for root in &self.children {
            rec(&root, &mut v);
        }

        v
    }

}

impl <T: Debug> std::fmt::Display for PatriciaTree<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        const INDENT_WIDTH: usize = 2;

        fn fmt_r<T: Debug>(f: &mut std::fmt::Formatter, node: &Node<T>, indent: usize) {
            for _ in 0..indent {
                write!(f, " ").unwrap();
            }
            writeln!(f, "- {} {:?}", node.prefix, node.value).unwrap();
            for c in &node.children {
                fmt_r(f, c, indent + INDENT_WIDTH);
            }
        }

        writeln!(f, "- (root)").unwrap();
        for root in &self.children {
            fmt_r(f, &root, INDENT_WIDTH);
        }
        write!(f, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let mut tree = PatriciaTree::<i32>::new();

        tree.add("aiueo", 1);
        assert_eq!(tree.get("aiueo"), Some(&1));
        assert_eq!(tree.get("aiue"), None);
        assert_eq!(tree.get("aiueoka"), None);

        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiueo".to_string(), Some(1))]);


        tree.add("aiuka".to_owned(), 2);
        assert_eq!(tree.get("aiueo".to_owned()), Some(&1));
        assert_eq!(tree.get("aiuka".to_owned()), Some(&2));

        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiu".to_string(), None), ("eo".to_string(), Some(1)), ("ka".to_string(), Some(2))]);

        tree.add("aiuk", 3);
        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiu".to_string(), None), ("eo".to_string(), Some(1)), ("k".to_string(), Some(3)), ("a".to_string(), Some(2)) ]);

        tree.add("aiueo", 4);
        assert_eq!(tree.get("aiueo"), Some(&4));
        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiu".to_string(), None), ("eo".to_string(), Some(4)), ("k".to_string(), Some(3)), ("a".to_string(), Some(2)) ]);

        tree.add("mentaiko", 8);
        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiu".to_string(), None), ("eo".to_string(), Some(4)), ("k".to_string(), Some(3)), ("a".to_string(), Some(2)), ("mentaiko".to_string(), Some(8)), ]);

        tree.add("hoge", 5);
        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiu".to_string(), None), ("eo".to_string(), Some(4)), ("k".to_string(), Some(3)), ("a".to_string(), Some(2)), ("hoge".to_string(), Some(5)), ("mentaiko".to_string(), Some(8)), ]);

        tree.delete("mentaiko");
        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiu".to_string(), None), ("eo".to_string(), Some(4)), ("k".to_string(), Some(3)), ("a".to_string(), Some(2)), ("hoge".to_string(), Some(5)), ]);

        tree.delete("aiuk");
        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiu".to_string(), None), ("eo".to_string(), Some(4)), ("ka".to_string(), Some(2)), ("hoge".to_string(), Some(5)), ]);

        tree.delete("aiueo");
        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![("aiuka".to_string(), Some(2)), ("hoge".to_string(), Some(5)), ]);

        tree.delete("hoge");
        tree.delete("aiuka");
        let nodes :Vec<(String, Option<i32>)> = tree.to_vec_preorder().iter().map(|n| (n.prefix.clone(), n.value)).collect();
        assert_eq!(nodes, vec![]);
    }
}
