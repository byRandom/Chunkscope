//! `Climate`: quantized climate points and the R-tree the game uses to pick
//! the biome whose parameter box is nearest to a sampled climate.

use std::cell::Cell;

/// `Climate.quantizeCoord`: `(long) (v * 10000f)`.
#[inline]
pub fn quantize(v: f32) -> i64 {
    (v * 10000.0) as i64
}

/// `Climate.TargetPoint`: a sampled, quantized climate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TargetPoint {
    pub temperature: i64,
    pub humidity: i64,
    pub continentalness: i64,
    pub erosion: i64,
    pub depth: i64,
    pub weirdness: i64,
}

impl TargetPoint {
    /// `TargetPoint.toParameterArray`: the 6 values plus a 0 for the offset axis.
    pub fn to_array(&self) -> [i64; 7] {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.depth,
            self.weirdness,
            0,
        ]
    }
}

/// `Climate.Parameter`: an inclusive quantized range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Parameter {
    pub min: i64,
    pub max: i64,
}

impl Parameter {
    pub const fn new(min: i64, max: i64) -> Self {
        Self { min, max }
    }

    /// `Parameter.distance(long)`: 0 inside the range, else distance to it.
    #[inline]
    pub fn distance(&self, v: i64) -> i64 {
        let above = v.wrapping_sub(self.max);
        let below = self.min.wrapping_sub(v);
        if above > 0 { above } else { below.max(0) }
    }

    /// `Parameter.span(Parameter)`.
    pub fn span(&self, other: &Parameter) -> Parameter {
        Parameter::new(self.min.min(other.min), self.max.max(other.max))
    }
}

/// `Climate.ParameterPoint`: the 6 ranges plus the `offset` axis, as the
/// 7-dimensional box used by the R-tree.
pub type ParameterSpace = [Parameter; 7];

#[derive(Clone, Copy, Debug)]
enum Node {
    Leaf {
        space: ParameterSpace,
        value: usize,
    },
    SubTree {
        space: ParameterSpace,
        children: (usize, usize),
    },
}

impl Node {
    fn space(&self) -> &ParameterSpace {
        match self {
            Node::Leaf { space, .. } | Node::SubTree { space, .. } => space,
        }
    }

    /// `RTree.Node.distance(long[])`.
    #[inline]
    fn distance(&self, target: &[i64; 7]) -> i64 {
        let mut d = 0i64;
        for (p, &t) in self.space().iter().zip(target) {
            let x = p.distance(t);
            d = d.wrapping_add(x.wrapping_mul(x));
        }
        d
    }
}

const CHILDREN_PER_NODE: usize = 19;

/// `Climate.RTree` + `Climate.ParameterList`: nearest-box search over
/// `(ParameterSpace, T)` entries.
///
/// Like the game, a search starts from the previous result (the game keeps
/// it in a `ThreadLocal`), and ties keep the earlier candidate. So when two
/// boxes are at exactly the same distance, the answer depends on the order of
/// previous queries. Use one `ParameterList` per sequence of queries you want
/// to reproduce.
pub struct ParameterList<T> {
    values: Vec<T>,
    nodes: Vec<Node>,
    /// Flattened child indices of every SubTree.
    child_index: Vec<usize>,
    root: usize,
    last_result: Cell<Option<usize>>,
}

impl<T: Clone> Clone for ParameterList<T> {
    fn clone(&self) -> Self {
        Self {
            values: self.values.clone(),
            nodes: self.nodes.clone(),
            child_index: self.child_index.clone(),
            root: self.root,
            last_result: Cell::new(None),
        }
    }
}

impl<T> ParameterList<T> {
    /// `new Climate.ParameterList(values)` (builds the R-tree like the game).
    pub fn new(entries: Vec<(ParameterSpace, T)>) -> Self {
        assert!(
            !entries.is_empty(),
            "Need at least one value to build the search tree."
        );
        let mut builder = Builder {
            nodes: Vec::new(),
            child_index: Vec::new(),
        };
        let mut values = Vec::with_capacity(entries.len());
        let leaves: Vec<usize> = entries
            .into_iter()
            .enumerate()
            .map(|(i, (space, value))| {
                values.push(value);
                builder.push(Node::Leaf { space, value: i })
            })
            .collect();
        let root = builder.build(leaves);
        Self {
            values,
            nodes: builder.nodes,
            child_index: builder.child_index,
            root,
            last_result: Cell::new(None),
        }
    }

    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Forget the previous result, as on a fresh game thread.
    pub fn reset(&self) {
        self.last_result.set(None);
    }

    /// `ParameterList.findValue` (R-tree search).
    pub fn find_value(&self, target: &TargetPoint) -> &T {
        let target = target.to_array();
        let leaf = self.search(self.root, &target, self.last_result.get());
        self.last_result.set(Some(leaf));
        match self.nodes[leaf] {
            Node::Leaf { value, .. } => &self.values[value],
            Node::SubTree { .. } => unreachable!(),
        }
    }

    /// `RTree.Node.search`: returns the index of the nearest leaf.
    fn search(&self, node: usize, target: &[i64; 7], best: Option<usize>) -> usize {
        match self.nodes[node] {
            Node::Leaf { .. } => node,
            Node::SubTree {
                children: (start, end),
                ..
            } => {
                let mut best = best;
                let mut best_distance = best.map_or(i64::MAX, |b| self.nodes[b].distance(target));
                for &child in &self.child_index[start..end] {
                    let d = self.nodes[child].distance(target);
                    if best_distance > d {
                        let found = self.search(child, target, best);
                        let found_distance = if found == child {
                            d
                        } else {
                            self.nodes[found].distance(target)
                        };
                        if best_distance > found_distance {
                            best_distance = found_distance;
                            best = Some(found);
                        }
                    }
                }
                best.expect("search from a non-empty subtree always finds a leaf")
            }
        }
    }
}

struct Builder {
    nodes: Vec<Node>,
    child_index: Vec<usize>,
}

impl Builder {
    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    fn space(&self, n: usize) -> &ParameterSpace {
        self.nodes[n].space()
    }

    /// `new RTree.SubTree(children)`: bounding box of the children.
    fn subtree(&mut self, children: Vec<usize>) -> usize {
        assert!(!children.is_empty(), "SubTree needs at least one child");
        let mut space = *self.space(children[0]);
        for &c in &children[1..] {
            let s = *self.space(c);
            for i in 0..7 {
                space[i] = space[i].span(&s[i]);
            }
        }
        let start = self.child_index.len();
        self.child_index.extend_from_slice(&children);
        let end = self.child_index.len();
        self.push(Node::SubTree {
            space,
            children: (start, end),
        })
    }

    fn children(&self, n: usize) -> Vec<usize> {
        match self.nodes[n] {
            Node::SubTree {
                children: (s, e), ..
            } => self.child_index[s..e].to_vec(),
            Node::Leaf { .. } => unreachable!(),
        }
    }

    /// `RTree.build(7, nodes, 19)`.
    fn build(&mut self, mut nodes: Vec<usize>) -> usize {
        assert!(!nodes.is_empty(), "Need at least one child to build a node");
        if nodes.len() == 1 {
            return nodes[0];
        }
        if nodes.len() <= CHILDREN_PER_NODE {
            nodes.sort_by_key(|&n| {
                self.space(n)
                    .iter()
                    .map(|p| ((p.min.wrapping_add(p.max)) / 2).abs())
                    .fold(0i64, |a, b| a.wrapping_add(b))
            });
            return self.subtree(nodes);
        }

        let mut best_cost = i64::MAX;
        let mut best_dimension = 0;
        let mut best_buckets: Vec<Vec<usize>> = Vec::new();
        for dimension in 0..7 {
            self.sort(&mut nodes, dimension, false);
            let buckets = bucketize(&nodes);
            let cost = buckets
                .iter()
                .map(|b| cost(&self.bounding_space(b)))
                .fold(0i64, |a, b| a.wrapping_add(b));
            if best_cost > cost {
                best_cost = cost;
                best_dimension = dimension;
                best_buckets = buckets;
            }
        }

        // The game wraps each bucket in a SubTree, sorts those, then rebuilds
        // each one's children recursively.
        let mut buckets: Vec<usize> = best_buckets.into_iter().map(|b| self.subtree(b)).collect();
        self.sort(&mut buckets, best_dimension, true);
        let children: Vec<usize> = buckets
            .into_iter()
            .map(|b| {
                let kids = self.children(b);
                self.build(kids)
            })
            .collect();
        self.subtree(children)
    }

    fn bounding_space(&self, nodes: &[usize]) -> ParameterSpace {
        let mut space = *self.space(nodes[0]);
        for &n in &nodes[1..] {
            let s = self.space(n);
            for i in 0..7 {
                space[i] = space[i].span(&s[i]);
            }
        }
        space
    }

    /// `RTree.sort`: stable sort by the center on `dimension`, then on the
    /// following dimensions (wrapping around).
    fn sort(&self, nodes: &mut [usize], dimension: usize, absolute: bool) {
        let key = |n: usize, d: usize| {
            let p = self.space(n)[d];
            let center = p.min.wrapping_add(p.max) / 2;
            if absolute { center.abs() } else { center }
        };
        nodes.sort_by(|&a, &b| {
            (0..7)
                .map(|i| (dimension + i) % 7)
                .map(|d| key(a, d).cmp(&key(b, d)))
                .find(|o| o.is_ne())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// `RTree.bucketize`: consecutive groups of `19^floor(log_19(n - 0.01))`.
fn bucketize(nodes: &[usize]) -> Vec<Vec<usize>> {
    let per_node = CHILDREN_PER_NODE as f64;
    let expected =
        per_node.powf(((nodes.len() as f64 - 0.01).ln() / per_node.ln()).floor()) as usize;
    nodes.chunks(expected).map(<[usize]>::to_vec).collect()
}

/// `RTree.cost`: sum of the box's side lengths.
fn cost(space: &ParameterSpace) -> i64 {
    space
        .iter()
        .map(|p| p.max.wrapping_sub(p.min).abs())
        .fold(0i64, |a, b| a.wrapping_add(b))
}
