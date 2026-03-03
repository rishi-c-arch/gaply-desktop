import React, { useEffect } from 'react';

interface SEOHeadProps {
  title?: string;
  description?: string;
  keywords?: string;
}

const SEOHead: React.FC<SEOHeadProps> = ({
  title = 'Gaply - AI-Powered Academic Research Platform',
  description = 'Professional AI-powered academic research platform for thesis writing, journal matching, and research paper assistance.',
  keywords = '',
}) => {
  useEffect(() => {
    document.title = title;
    const metaDescription = document.querySelector('meta[name="description"]');
    if (metaDescription) metaDescription.setAttribute('content', description);
    const metaKeywords = document.querySelector('meta[name="keywords"]');
    if (metaKeywords && keywords) metaKeywords.setAttribute('content', keywords);
    const ogTitle = document.querySelector('meta[property="og:title"]');
    if (ogTitle) ogTitle.setAttribute('content', title);
    const ogDesc = document.querySelector('meta[property="og:description"]');
    if (ogDesc) ogDesc.setAttribute('content', description);
  }, [title, description, keywords]);
  return null;
};

export default SEOHead;
