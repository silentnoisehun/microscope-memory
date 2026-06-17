# Microscope Memory - Streamlit Dashboard
import streamlit as st
import requests

API = "http://localhost:6060/v1"
st.set_page_config(page_title="Microscope Memory Explorer", layout="wide")
st.title("🔬 Microscope Memory Explorer")

col1, col2, col3 = st.columns(3)

with col1:
    st.subheader("Status")
    try:
        r = requests.get(f"{API}/status", timeout=2)
        if r.ok:
            d = r.json()
            st.metric("Blocks", d.get("blocks", 0))
            st.metric("Version", d.get("version", ""))
    except:
        st.error("Bridge not running")

with col2:
    st.subheader("Store")
    text = st.text_area("Memory text")
    imp = st.slider("Importance", 1, 10, 5)
    if st.button("Store"):
        r = requests.post(f"{API}/remember", json={"text": text, "importance": imp})
        st.success("Stored!" if r.ok else "Failed")

with col3:
    st.subheader("Recall")
    q = st.text_input("Query")
    k = st.slider("Results", 1, 20, 5)
    if st.button("Recall"):
        r = requests.get(f"{API}/recall", params={"q": q, "k": k})
        if r.ok:
            for m in (r.json() or []):
                st.write(f"- {m.get("text","")}")

# pip install streamlit requests && streamlit run streamlit_app.py
