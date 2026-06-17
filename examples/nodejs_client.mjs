// Microscope Memory - Node.js client
import axios from "axios";
const API = "http://localhost:6060/v1";
export async function recall(query, k = 10) {
  const r = await axios.get(`${API}/recall`, { params: { q: query, k } });
  return r.data || [];
}
export async function store(text, layer = "long_term", imp = 5) {
  const r = await axios.post(`${API}/remember`, { text, layer, importance: imp });
  return r.status === 200;
}
// Usage:
// const mems = await recall("JavaScript");
// console.log(mems);
