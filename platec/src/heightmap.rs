//! The 2D buffer used for the height, age and plate-index maps.
//!
//! [`Matrix`] owns its buffer: [`Matrix::new`] allocates it zero-initialised,
//! and [`Matrix::from_vec`] takes ownership of one the caller has built.

use crate::platec_assert;
use std::ops::{Index, IndexMut};

#[derive(Clone, Debug)]
pub struct Matrix<T> {
    data: Vec<T>,
    width: u32,
    height: u32,
}

impl<T: Clone + Default> Matrix<T> {
    pub fn new(width: u32, height: u32) -> Self {
        platec_assert!(
            width != 0 && height != 0,
            "Matrix width and height should be greater than zero"
        );
        let area = (width as usize) * (height as usize);
        Self {
            data: vec![T::default(); area],
            width,
            height,
        }
    }
}

impl<T: Clone> Matrix<T> {
    /// Take ownership of an existing buffer.
    pub fn from_vec(data: Vec<T>, width: u32, height: u32) -> Self {
        platec_assert!(
            !data.is_empty() && width != 0 && height != 0,
            "Invalid matrix data"
        );
        debug_assert_eq!(data.len(), (width as usize) * (height as usize));
        Self {
            data,
            width,
            height,
        }
    }

    pub fn set_all(&mut self, value: T) {
        for slot in self.data.iter_mut() {
            *slot = value.clone();
        }
    }

    /// Reallocates (and adopts the other's dimensions)
    /// when the areas differ.
    pub fn copy_from(&mut self, other: &Matrix<T>) {
        if self.data.len() != other.data.len() {
            self.width = other.width;
            self.height = other.height;
        }
        self.data.clone_from(&other.data);
    }

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, value: T) {
        platec_assert!(x < self.width && y < self.height, "Invalid coordinates");
        let idx = (y as usize) * (self.width as usize) + (x as usize);
        self.data[idx] = value;
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32) -> &T {
        platec_assert!(x < self.width && y < self.height, "Invalid coordinates");
        &self.data[(y as usize) * (self.width as usize) + (x as usize)]
    }

    pub fn as_slice(&self) -> &[T] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }

    /// Consume the matrix, returning the underlying buffer.
    pub fn into_vec(self) -> Vec<T> {
        self.data
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    #[inline]
    pub fn area(&self) -> u32 {
        self.width.wrapping_mul(self.height)
    }
}

impl<T> Index<usize> for Matrix<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        &self.data[index]
    }
}

impl<T> IndexMut<usize> for Matrix<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.data[index]
    }
}

impl<T> Index<u32> for Matrix<T> {
    type Output = T;
    fn index(&self, index: u32) -> &T {
        &self.data[index as usize]
    }
}

impl<T> IndexMut<u32> for Matrix<T> {
    fn index_mut(&mut self, index: u32) -> &mut T {
        &mut self.data[index as usize]
    }
}

pub type HeightMap = Matrix<f32>;
pub type AgeMap = Matrix<u32>;
pub type IndexMap = Matrix<u32>;
