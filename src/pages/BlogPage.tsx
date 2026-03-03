import React, { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { BLOG_POSTS } from '../data/blogPosts';
import './BlogPage.css';

const BlogPage: React.FC = () => {
  const [currentPage, setCurrentPage] = useState(1);
  const [showUploadForm, setShowUploadForm] = useState(false);
  const [inView, setInView] = useState(false);
  const postsPerPage = 5;
  const totalPages = Math.ceil(BLOG_POSTS.length / postsPerPage);
  const startIdx = (currentPage - 1) * postsPerPage;
  const visiblePosts = BLOG_POSTS.slice(startIdx, startIdx + postsPerPage);

  useEffect(() => {
    setInView(true);
  }, []);

  return (
    <div className="blog-page">
      <div className="blog-page__bg" aria-hidden="true" />
      <header className="blog-page__header">
        <Link to="/" className="blog-page__back">← Back to Gaply</Link>
        <h1 className="blog-page__title">Gaply Blog</h1>
        <p className="blog-page__subtitle">
          Insights on research writing, thesis guidance, and academic publishing.
        </p>
        <button
          type="button"
          className="blog-page__upload-btn"
          onClick={() => setShowUploadForm((v) => !v)}
        >
          {showUploadForm ? 'Hide Upload Form' : '+ Upload a Blog'}
        </button>
      </header>

      {showUploadForm && (
        <section className="blog-upload glass-card">
          <h2 className="blog-upload__title">Upload a Blog (with SEO)</h2>
          <form
            className="blog-upload__form"
            onSubmit={(e) => {
              e.preventDefault();
              alert('Blog submission received! (Backend integration coming soon.)');
            }}
          >
            <div className="blog-upload__row">
              <label>Title</label>
              <input name="title" placeholder="Blog title" required />
            </div>
            <div className="blog-upload__row">
              <label>Meta Title (SEO)</label>
              <input name="metaTitle" placeholder="50–60 chars for search" />
            </div>
            <div className="blog-upload__row">
              <label>Meta Description (SEO)</label>
              <textarea name="metaDescription" placeholder="150–160 chars" rows={2} />
            </div>
            <div className="blog-upload__row">
              <label>SEO Keywords (comma-separated)</label>
              <input name="keywords" placeholder="research paper, thesis writing, journal matching" />
            </div>
            <div className="blog-upload__row">
              <label>Content</label>
              <textarea name="content" placeholder="Your blog content..." rows={6} required />
            </div>
            <button type="submit" className="blog-upload__submit">Submit Blog</button>
          </form>
        </section>
      )}

      <main className="blog-page__main">
        <div className="blog-feed">
          {visiblePosts.map((post, index) => (
            <article
              key={post.id}
              className={`blog-card glass-card ${inView ? 'blog-card--visible' : ''}`}
              style={{ transitionDelay: `${index * 80}ms` }}
            >
              <Link to={`/blog/${post.slug}`} className="blog-card__link">
                <div className="blog-card__inner">
                  <div
                    className="blog-card__image"
                    style={{ backgroundImage: post.imageUrl ? `url(${post.imageUrl})` : undefined }}
                  />
                  <div className="blog-card__content">
                    <h2 className="blog-card__title">{post.title}</h2>
                    <p className="blog-card__excerpt">{post.intro}</p>
                    <div className="blog-card__meta">
                      <span className="blog-card__date">
                        {new Date(post.date).toLocaleDateString('en-US', {
                          month: 'short',
                          day: 'numeric',
                          year: 'numeric',
                        })}{' '}
                        • {post.readTime}
                      </span>
                      <span className="blog-card__cta">
                        Read Entry <span className="blog-card__arrow">→</span>
                      </span>
                    </div>
                  </div>
                </div>
              </Link>
            </article>
          ))}
        </div>

        {totalPages > 1 && (
          <nav className="blog-pagination" aria-label="Blog pagination">
            <button
              type="button"
              className="blog-pagination__btn"
              onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
              disabled={currentPage === 1}
            >
              ←
            </button>
            {Array.from({ length: totalPages }, (_, i) => i + 1).map((p) => (
              <button
                key={p}
                type="button"
                className={`blog-pagination__btn ${p === currentPage ? 'blog-pagination__btn--active' : ''}`}
                onClick={() => setCurrentPage(p)}
              >
                {p}
              </button>
            ))}
            <button
              type="button"
              className="blog-pagination__btn"
              onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
              disabled={currentPage === totalPages}
            >
              →
            </button>
          </nav>
        )}
      </main>
    </div>
  );
};

export default BlogPage;
