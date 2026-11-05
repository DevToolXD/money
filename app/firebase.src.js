/* 파이어베이스 라이브러리(구글이 JS로만 주는 것)를 러스트에 넘겨 주는 목록이에요. 우리 코드(로그인·지갑·대화·회사…)는 모두 러스트예요: web/crates/ui/src/fire.rs
   러스트(WASM)는 npm 라이브러리를 직접 불러올 수 없어서, 여기서 불러온 함수를 그대로 window.fb에 담아 두기만 해요. 계산이나 판단은 하지 않아요.
   `npm run build`(tests 폴더)가 이 파일을 app/firebase.js 한 개로 묶어요. */
import { firebaseConfig } from "./firebase-config.js";
import { initializeApp } from "firebase/app";
import {
  initializeAuth, connectAuthEmulator, indexedDBLocalPersistence, browserLocalPersistence, onAuthStateChanged,
  createUserWithEmailAndPassword, signInWithEmailAndPassword, signOut, updateProfile, deleteUser,
} from "firebase/auth";
import {
  initializeFirestore, connectFirestoreEmulator, memoryLocalCache, doc, getDoc, setDoc, updateDoc, collection, query, where, orderBy,
  limit, startAfter, onSnapshot, getDocs, getCountFromServer, documentId, writeBatch, runTransaction, serverTimestamp, increment, Timestamp, deleteField,
} from "firebase/firestore";

window.fb = {
  config: firebaseConfig,
  emu: __EMU__,
  initializeApp, initializeAuth, connectAuthEmulator, indexedDBLocalPersistence, browserLocalPersistence, onAuthStateChanged,
  createUserWithEmailAndPassword, signInWithEmailAndPassword, signOut, updateProfile, deleteUser,
  initializeFirestore, connectFirestoreEmulator, memoryLocalCache, doc, getDoc, setDoc, updateDoc, collection, query, where, orderBy,
  limit, startAfter, onSnapshot, getDocs, getCountFromServer, documentId, writeBatch, runTransaction, serverTimestamp, increment, Timestamp, deleteField,
};
